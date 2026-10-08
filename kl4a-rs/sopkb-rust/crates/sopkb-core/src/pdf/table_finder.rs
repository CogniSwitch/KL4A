//! Port of pdfplumber 0.11.10's `TableFinder` (`pdfplumber/table.py`) and the
//! geometry helpers it depends on (`pdfplumber/utils/geometry.py`), restricted
//! to the default `vertical_strategy="lines"`/`horizontal_strategy="lines"`
//! settings -- the only ones oss-launch's `normalize.py` ever uses
//! (`page.find_tables()`, no custom `table_settings`). The `"text"`/
//! `"explicit"` strategies are deliberately not ported: nothing in this port's
//! call path can reach them.
//!
//! Read from the actual installed `pdfplumber==0.11.10` package (the same
//! version oss-launch's `pyproject.toml` pins via `pdfplumber>=0.11`), not
//! reimplemented from memory or documentation -- every function below has a
//! named Python counterpart and mirrors its algorithm line-for-line, tolerance
//! constants included.
//!
//! Pipeline (`TableFinder.__init__`): page graphics -> [`edges`] ->
//! [`edges_to_intersections`] -> [`intersections_to_cells`] ->
//! [`cells_to_tables`] -> one [`Table`] per contiguous cell group.

use super::graphics::GraphicsObj;
use super::words::{self, cluster_objects, PdfChar, WordExtractor};

const DEFAULT_SNAP_TOLERANCE: f64 = 3.0;
const DEFAULT_JOIN_TOLERANCE: f64 = 3.0;
const EDGE_MIN_LENGTH_PREFILTER: f64 = 1.0;
const EDGE_MIN_LENGTH: f64 = 3.0;
const INTERSECTION_TOLERANCE: f64 = 3.0;

/// One `pdfplumber` edge dict, restricted to the keys this port reads.
/// `orientation` is `None` for a diagonal curve segment (pdfminer's
/// `curve_to_edges` leaves `orientation` unset when neither endpoint shares an
/// x nor a y) -- such an edge matches neither the `"v"` nor `"h"` filter in
/// [`get_edges`], so it is silently dropped, exactly as real pdfplumber drops
/// it via `filter_edges`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Edge {
    x0: f64,
    x1: f64,
    top: f64,
    bottom: f64,
    orientation: Option<char>,
}

/// `rect_to_edges`/`line_to_edge`/`curve_to_edges` (`geometry.py`), fused into
/// one dispatch over [`GraphicsObj`] the way pdfplumber's own `obj_to_edges`
/// dispatches on `object_type`.
fn obj_to_edges(obj: &GraphicsObj) -> Vec<Edge> {
    match obj {
        GraphicsObj::Rect { x0, top, x1, bottom } => vec![
            Edge { x0: *x0, x1: *x1, top: *top, bottom: *top, orientation: Some('h') },
            Edge { x0: *x0, x1: *x1, top: *bottom, bottom: *bottom, orientation: Some('h') },
            Edge { x0: *x0, x1: *x0, top: *top, bottom: *bottom, orientation: Some('v') },
            Edge { x0: *x1, x1: *x1, top: *top, bottom: *bottom, orientation: Some('v') },
        ],
        GraphicsObj::Line { x0, top, x1, bottom } => {
            // `line_to_edge`: orientation decided by exact top==bottom, not shape.
            let orientation = if (*top - *bottom).abs() == 0.0 { 'h' } else { 'v' };
            vec![Edge { x0: *x0, x1: *x1, top: *top, bottom: *bottom, orientation: Some(orientation) }]
        }
        GraphicsObj::Curve { pts, .. } => pts
            .windows(2)
            .map(|w| {
                let (p0, p1) = (w[0], w[1]);
                let orientation =
                    if p0.0 == p1.0 { Some('v') } else if p0.1 == p1.1 { Some('h') } else { None };
                Edge {
                    x0: p0.0.min(p1.0),
                    x1: p0.0.max(p1.0),
                    top: p0.1.min(p1.1),
                    bottom: p0.1.max(p1.1),
                    orientation,
                }
            })
            .collect(),
    }
}

/// `filter_edges`: `dim = height if orientation == "v" else width`.
fn filter_edges(edges: &[Edge], orientation: Option<char>, min_length: f64) -> Vec<Edge> {
    edges
        .iter()
        .copied()
        .filter(|e| {
            let Some(o) = e.orientation else { return false };
            if let Some(want) = orientation {
                if o != want {
                    return false;
                }
            }
            let dim = if o == 'v' { e.bottom - e.top } else { e.x1 - e.x0 };
            dim >= min_length
        })
        .collect()
}

/// `move_object`/`snap_objects`: shift every edge in a cluster onto that
/// cluster's positional average. `axis` mirrors Python's own inversion --
/// clustering **v** edges by `x0` moves them along the **h** (horizontal)
/// axis, and vice versa for **h** edges clustered by `top`.
fn snap_objects(edges: &[Edge], key_fn: impl Fn(&Edge) -> f64, tolerance: f64, shift_x: bool) -> Vec<Edge> {
    let clusters = cluster_objects(edges, &key_fn, tolerance);
    let mut out = Vec::new();
    for cluster in clusters {
        let avg = cluster.iter().map(|e| key_fn(e)).sum::<f64>() / cluster.len() as f64;
        for &e in &cluster {
            let diff = avg - key_fn(e);
            let mut moved = *e;
            if shift_x {
                moved.x0 += diff;
                moved.x1 += diff;
            } else {
                moved.top += diff;
                moved.bottom += diff;
            }
            out.push(moved);
        }
    }
    out
}

/// `snap_edges`.
fn snap_edges(edges: &[Edge], x_tolerance: f64, y_tolerance: f64) -> Vec<Edge> {
    let v: Vec<Edge> = edges.iter().copied().filter(|e| e.orientation == Some('v')).collect();
    let h: Vec<Edge> = edges.iter().copied().filter(|e| e.orientation == Some('h')).collect();
    let mut snapped_v = snap_objects(&v, |e| e.x0, x_tolerance, true);
    let snapped_h = snap_objects(&h, |e| e.top, y_tolerance, false);
    snapped_v.extend(snapped_h);
    snapped_v
}

/// `join_edge_group`: merge nearly-touching colinear segments along the same
/// infinite line into fewer, longer ones.
fn join_edge_group(mut edges: Vec<Edge>, orientation: char, tolerance: f64) -> Vec<Edge> {
    if orientation == 'h' {
        edges.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap());
    } else {
        edges.sort_by(|a, b| a.top.partial_cmp(&b.top).unwrap());
    }
    let mut joined: Vec<Edge> = vec![edges[0]];
    for &e in &edges[1..] {
        let last = joined.last_mut().unwrap();
        if orientation == 'h' {
            if e.x0 <= last.x1 + tolerance {
                if e.x1 > last.x1 {
                    last.x1 = e.x1;
                }
            } else {
                joined.push(e);
            }
        } else if e.top <= last.bottom + tolerance {
            if e.bottom > last.bottom {
                last.bottom = e.bottom;
            }
        } else {
            joined.push(e);
        }
    }
    joined
}

/// `merge_edges`: snap, then group by `(orientation, exact snapped position)`
/// and join each group.
fn merge_edges(edges: Vec<Edge>, snap_x: f64, snap_y: f64, join_x: f64, join_y: f64) -> Vec<Edge> {
    let edges = if snap_x > 0.0 || snap_y > 0.0 { snap_edges(&edges, snap_x, snap_y) } else { edges };

    let mut keyed: Vec<(char, f64, Edge)> = edges
        .iter()
        .filter_map(|e| e.orientation.map(|o| (o, if o == 'h' { e.top } else { e.x0 }, *e)))
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap()));

    let mut out = Vec::new();
    let mut i = 0;
    while i < keyed.len() {
        let (o, k) = (keyed[i].0, keyed[i].1);
        let mut j = i;
        let mut group = Vec::new();
        while j < keyed.len() && keyed[j].0 == o && keyed[j].1 == k {
            group.push(keyed[j].2);
            j += 1;
        }
        let tol = if o == 'h' { join_x } else { join_y };
        out.extend(join_edge_group(group, o, tol));
        i = j;
    }
    out
}

/// `TableFinder.get_edges`, restricted to the `"lines"`/`"lines"` default
/// strategy (see module docs).
fn get_edges(objs: &[GraphicsObj]) -> Vec<Edge> {
    let all: Vec<Edge> = objs.iter().flat_map(obj_to_edges).collect();
    let v = filter_edges(&all, Some('v'), EDGE_MIN_LENGTH_PREFILTER);
    let h = filter_edges(&all, Some('h'), EDGE_MIN_LENGTH_PREFILTER);
    let mut combined = v;
    combined.extend(h);
    let merged = merge_edges(
        combined,
        DEFAULT_SNAP_TOLERANCE,
        DEFAULT_SNAP_TOLERANCE,
        DEFAULT_JOIN_TOLERANCE,
        DEFAULT_JOIN_TOLERANCE,
    );
    filter_edges(&merged, None, EDGE_MIN_LENGTH)
}

#[derive(Default)]
struct Intersection {
    v: Vec<Edge>,
    h: Vec<Edge>,
}

/// `edges_to_intersections`: every (v, h) pair that cross within tolerance
/// contributes a point, keyed by `(v.x0, h.top)`.
fn edges_to_intersections(edges: &[Edge], x_tolerance: f64, y_tolerance: f64) -> Vec<((f64, f64), Intersection)> {
    let mut out: Vec<((f64, f64), Intersection)> = Vec::new();
    let v_edges: Vec<Edge> = edges.iter().copied().filter(|e| e.orientation == Some('v')).collect();
    let h_edges: Vec<Edge> = edges.iter().copied().filter(|e| e.orientation == Some('h')).collect();
    for v in &v_edges {
        for h in &h_edges {
            if v.top <= h.top + y_tolerance
                && v.bottom >= h.top - y_tolerance
                && v.x0 >= h.x0 - x_tolerance
                && v.x0 <= h.x1 + x_tolerance
            {
                let vertex = (v.x0, h.top);
                match out.iter_mut().find(|(p, _)| *p == vertex) {
                    Some((_, entry)) => {
                        entry.v.push(*v);
                        entry.h.push(*h);
                    }
                    None => {
                        out.push((vertex, Intersection { v: vec![*v], h: vec![*h] }));
                    }
                }
            }
        }
    }
    out
}

fn edge_bits(e: &Edge) -> (u64, u64, u64, u64) {
    (e.x0.to_bits(), e.top.to_bits(), e.x1.to_bits(), e.bottom.to_bits())
}

/// `intersections_to_cells`: for each point, look for the smallest rectangle
/// whose four corners are all known intersections and whose four sides are
/// each covered by one continuous edge (checked via `edge_connects`, i.e. two
/// points share an actual edge OBJECT, not merely "both have some v/h edge").
fn intersections_to_cells(intersections: &[((f64, f64), Intersection)]) -> Vec<(f64, f64, f64, f64)> {
    let get = |p: (f64, f64)| -> Option<&Intersection> { intersections.iter().find(|(k, _)| *k == p).map(|(_, v)| v) };

    let edge_connects = |p1: (f64, f64), p2: (f64, f64)| -> bool {
        if p1.0 == p2.0 {
            let (Some(i1), Some(i2)) = (get(p1), get(p2)) else { return false };
            let s1: std::collections::HashSet<_> = i1.v.iter().map(edge_bits).collect();
            if i2.v.iter().any(|e| s1.contains(&edge_bits(e))) {
                return true;
            }
        }
        if p1.1 == p2.1 {
            let (Some(i1), Some(i2)) = (get(p1), get(p2)) else { return false };
            let s1: std::collections::HashSet<_> = i1.h.iter().map(edge_bits).collect();
            if i2.h.iter().any(|e| s1.contains(&edge_bits(e))) {
                return true;
            }
        }
        false
    };

    let mut points: Vec<(f64, f64)> = intersections.iter().map(|(k, _)| *k).collect();
    points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap()));

    let n = points.len();
    let mut cells = Vec::new();
    for i in 0..n {
        if i == n - 1 {
            break;
        }
        let pt = points[i];
        let rest = &points[i + 1..];
        let below: Vec<(f64, f64)> = rest.iter().copied().filter(|p| p.0 == pt.0).collect();
        let right: Vec<(f64, f64)> = rest.iter().copied().filter(|p| p.1 == pt.1).collect();
        'search: for &below_pt in &below {
            if !edge_connects(pt, below_pt) {
                continue;
            }
            for &right_pt in &right {
                if !edge_connects(pt, right_pt) {
                    continue;
                }
                let bottom_right = (right_pt.0, below_pt.1);
                if get(bottom_right).is_some()
                    && edge_connects(bottom_right, right_pt)
                    && edge_connects(bottom_right, below_pt)
                {
                    cells.push((pt.0, pt.1, bottom_right.0, bottom_right.1));
                    break 'search;
                }
            }
        }
    }
    cells
}

/// `cells_to_tables`: greedily group cells that share at least one corner
/// into contiguous tables, then drop any single-cell "table" (matching
/// `filtered = [t for t in _sorted if len(t) > 1]`).
fn cells_to_tables(cells: &[(f64, f64, f64, f64)]) -> Vec<Vec<(f64, f64, f64, f64)>> {
    fn corners(c: (f64, f64, f64, f64)) -> [(u64, u64); 4] {
        let (x0, top, x1, bottom) = c;
        [(x0.to_bits(), top.to_bits()), (x0.to_bits(), bottom.to_bits()), (x1.to_bits(), top.to_bits()), (x1.to_bits(), bottom.to_bits())]
    }

    let mut remaining: Vec<(f64, f64, f64, f64)> = cells.to_vec();
    let mut current_corners: std::collections::HashSet<(u64, u64)> = Default::default();
    let mut current_cells: Vec<(f64, f64, f64, f64)> = Vec::new();
    let mut tables: Vec<Vec<(f64, f64, f64, f64)>> = Vec::new();

    while !remaining.is_empty() {
        let initial_count = current_cells.len();
        let mut i = 0;
        while i < remaining.len() {
            let cell = remaining[i];
            let cell_corners = corners(cell);
            let should_take = current_cells.is_empty() || cell_corners.iter().any(|c| current_corners.contains(c));
            if should_take {
                current_corners.extend(cell_corners);
                current_cells.push(cell);
                remaining.remove(i);
            } else {
                i += 1;
            }
        }
        if current_cells.len() == initial_count {
            tables.push(std::mem::take(&mut current_cells));
            current_corners.clear();
        }
    }
    if !current_cells.is_empty() {
        tables.push(current_cells);
    }

    // Sort top-to-bottom-left-to-right by each table's own minimum (top, x0).
    tables.sort_by(|a, b| {
        let ka = a.iter().map(|c| (c.1, c.0)).min_by(|x, y| x.partial_cmp(y).unwrap()).unwrap();
        let kb = b.iter().map(|c| (c.1, c.0)).min_by(|x, y| x.partial_cmp(y).unwrap()).unwrap();
        ka.partial_cmp(&kb).unwrap()
    });
    tables.into_iter().filter(|t| t.len() > 1).collect()
}

/// `CellGroup`/`Row`/`Column`: a table row or column, as a full grid slot list
/// aligned to the table's global distinct column (row) positions -- `None`
/// where this row (column) has no cell at that position, matching pdfplumber's
/// sparse-grid support.
pub struct CellGroup {
    pub cells: Vec<Option<(f64, f64, f64, f64)>>,
    pub bbox: (f64, f64, f64, f64),
}

fn cellgroup_bbox(cells: &[Option<(f64, f64, f64, f64)>]) -> (f64, f64, f64, f64) {
    let present: Vec<(f64, f64, f64, f64)> = cells.iter().filter_map(|c| *c).collect();
    (
        present.iter().map(|c| c.0).fold(f64::INFINITY, f64::min),
        present.iter().map(|c| c.1).fold(f64::INFINITY, f64::min),
        present.iter().map(|c| c.2).fold(f64::NEG_INFINITY, f64::max),
        present.iter().map(|c| c.3).fold(f64::NEG_INFINITY, f64::max),
    )
}

/// A detected table: one contiguous group of cells, in `(x0, top, x1, bottom)`
/// form. Mirrors pdfplumber's `Table`.
pub struct Table {
    pub cells: Vec<(f64, f64, f64, f64)>,
}

impl Table {
    pub fn bbox(&self) -> (f64, f64, f64, f64) {
        (
            self.cells.iter().map(|c| c.0).fold(f64::INFINITY, f64::min),
            self.cells.iter().map(|c| c.1).fold(f64::INFINITY, f64::min),
            self.cells.iter().map(|c| c.2).fold(f64::NEG_INFINITY, f64::max),
            self.cells.iter().map(|c| c.3).fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// `Table._get_rows_or_cols`. For rows: group by `top`, columns ordered by
    /// the table's distinct `x0` positions. For columns: group by `x0`, rows
    /// ordered by the table's distinct `top` positions.
    fn rows_or_cols(&self, want_rows: bool) -> Vec<CellGroup> {
        let axis = |c: &(f64, f64, f64, f64)| if want_rows { c.0 } else { c.1 };
        let antiaxis = |c: &(f64, f64, f64, f64)| if want_rows { c.1 } else { c.0 };

        let mut sorted = self.cells.clone();
        sorted.sort_by(|a, b| antiaxis(a).partial_cmp(&antiaxis(b)).unwrap().then(axis(a).partial_cmp(&axis(b)).unwrap()));

        let mut xs: Vec<f64> = self.cells.iter().map(axis).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup_by(|a, b| *a == *b);

        let mut groups: Vec<CellGroup> = Vec::new();
        let mut i = 0;
        while i < sorted.len() {
            let key = antiaxis(&sorted[i]);
            let mut j = i;
            let mut members: Vec<(f64, (f64, f64, f64, f64))> = Vec::new();
            while j < sorted.len() && antiaxis(&sorted[j]) == key {
                members.push((axis(&sorted[j]), sorted[j]));
                j += 1;
            }
            // `{cell[axis]: cell for cell in row_cells}` -- last write wins.
            let cells_for_group: Vec<Option<(f64, f64, f64, f64)>> =
                xs.iter().map(|&x| members.iter().rev().find(|(ax, _)| *ax == x).map(|(_, c)| *c)).collect();
            let bbox = cellgroup_bbox(&cells_for_group);
            groups.push(CellGroup { cells: cells_for_group, bbox });
            i = j;
        }
        groups
    }

    pub fn rows(&self) -> Vec<CellGroup> {
        self.rows_or_cols(true)
    }

    pub fn columns(&self) -> Vec<CellGroup> {
        self.rows_or_cols(false)
    }

    /// `Table.extract`: for each row, find the chars whose midpoint falls in
    /// the row's own bbox, then for each cell in that row, find which of
    /// those chars fall in the cell's own bbox and render them as text.
    /// `None` for a sparse-grid slot with no cell, `""` for an empty cell.
    pub fn extract(&self, chars: &[PdfChar]) -> Vec<Vec<Option<String>>> {
        let mut out = Vec::new();
        for row in self.rows() {
            let row_chars: Vec<&PdfChar> = chars.iter().filter(|c| char_in_bbox(c, row.bbox)).collect();
            let mut arr = Vec::new();
            for cell in &row.cells {
                arr.push(match cell {
                    None => None,
                    Some(bbox) => {
                        let cell_chars: Vec<PdfChar> =
                            row_chars.iter().filter(|c| char_in_bbox(c, *bbox)).map(|c| (*c).clone()).collect();
                        Some(if cell_chars.is_empty() {
                            String::new()
                        } else {
                            words::extract_text(&cell_chars, &WordExtractor::default())
                        })
                    }
                });
            }
            out.push(arr);
        }
        out
    }
}

fn char_in_bbox(c: &PdfChar, bbox: (f64, f64, f64, f64)) -> bool {
    let v_mid = (c.top + c.bottom) / 2.0;
    let h_mid = (c.x0 + c.x1) / 2.0;
    let (x0, top, x1, bottom) = bbox;
    h_mid >= x0 && h_mid < x1 && v_mid >= top && v_mid < bottom
}

/// `page.find_tables()` (default settings): the full pipeline from a page's
/// vector graphics to a list of detected tables.
pub fn find_tables(objs: &[GraphicsObj]) -> Vec<Table> {
    let edges = get_edges(objs);
    let intersections = edges_to_intersections(&edges, INTERSECTION_TOLERANCE, INTERSECTION_TOLERANCE);
    let cells = intersections_to_cells(&intersections);
    cells_to_tables(&cells).into_iter().map(|cells| Table { cells }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, top: f64, x1: f64, bottom: f64) -> GraphicsObj {
        GraphicsObj::Rect { x0, top, x1, bottom }
    }

    fn line(x0: f64, top: f64, x1: f64, bottom: f64) -> GraphicsObj {
        GraphicsObj::Line { x0, top, x1, bottom }
    }

    fn ch(text: &str, x0: f64, x1: f64, top: f64, bottom: f64) -> PdfChar {
        PdfChar { text: text.into(), x0, x1, top, bottom, doctop: top, upright: true, size: bottom - top, fontname: "F".into() }
    }

    /// A 2x2 grid built from 4 rect "cell boxes" sharing edges, the way a
    /// bordered table is commonly drawn (each cell its own stroked rect,
    /// abutting its neighbors) -- exercises snap/join merging touching edges
    /// from adjacent rects into one shared line.
    #[test]
    fn two_by_two_grid_of_abutting_rects_is_one_table_with_four_cells() {
        let objs = vec![
            rect(0.0, 0.0, 50.0, 20.0),
            rect(50.0, 0.0, 100.0, 20.0),
            rect(0.0, 20.0, 50.0, 40.0),
            rect(50.0, 20.0, 100.0, 40.0),
        ];
        let tables = find_tables(&objs);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].cells.len(), 4);
        assert_eq!(tables[0].bbox(), (0.0, 0.0, 100.0, 40.0));
        assert_eq!(tables[0].rows().len(), 2);
        assert_eq!(tables[0].columns().len(), 2);
    }

    /// The same grid drawn as ruling lines (3 horizontal + 3 vertical),
    /// pdfplumber's other common real-world table style.
    #[test]
    fn two_by_two_grid_of_ruling_lines_is_one_table_with_four_cells() {
        let objs = vec![
            line(0.0, 0.0, 100.0, 0.0),
            line(0.0, 20.0, 100.0, 20.0),
            line(0.0, 40.0, 100.0, 40.0),
            line(0.0, 0.0, 0.0, 40.0),
            line(50.0, 0.0, 50.0, 40.0),
            line(100.0, 0.0, 100.0, 40.0),
        ];
        let tables = find_tables(&objs);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].cells.len(), 4);
    }

    #[test]
    fn nearly_touching_lines_within_snap_tolerance_still_form_a_table() {
        // Same grid, but every shared line is drawn as two independent
        // segments 1pt apart (a common dashed/re-drawn-border artifact) --
        // must still snap+join into one continuous edge per line.
        let objs = vec![
            line(0.0, 0.0, 49.0, 0.0),
            line(51.0, 1.0, 100.0, 1.0),
            line(0.0, 20.0, 100.0, 20.0),
            line(0.0, 40.0, 100.0, 40.0),
            line(0.0, 0.0, 0.0, 40.0),
            line(50.0, 0.0, 50.0, 40.0),
            line(100.0, 0.0, 100.0, 40.0),
        ];
        let tables = find_tables(&objs);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].cells.len(), 4);
    }

    #[test]
    fn a_single_isolated_cell_is_not_a_table() {
        // One rect alone forms exactly one cell with no neighbor to share a
        // corner with -- `cells_to_tables` must drop it (len > 1 filter).
        let objs = vec![rect(0.0, 0.0, 50.0, 20.0)];
        assert!(find_tables(&objs).is_empty());
    }

    #[test]
    fn unrelated_lines_with_no_crossing_produce_no_table() {
        let objs = vec![line(0.0, 0.0, 100.0, 0.0), line(0.0, 100.0, 100.0, 100.0)];
        assert!(find_tables(&objs).is_empty());
    }

    #[test]
    fn extract_reads_chars_from_their_own_cell() {
        let objs = vec![
            rect(0.0, 0.0, 50.0, 20.0),
            rect(50.0, 0.0, 100.0, 20.0),
            rect(0.0, 20.0, 50.0, 40.0),
            rect(50.0, 20.0, 100.0, 40.0),
        ];
        let tables = find_tables(&objs);
        let chars = vec![
            ch("A", 5.0, 15.0, 5.0, 15.0),
            ch("B", 60.0, 70.0, 5.0, 15.0),
            ch("C", 5.0, 15.0, 25.0, 35.0),
            ch("D", 60.0, 70.0, 25.0, 35.0),
        ];
        let extracted = tables[0].extract(&chars);
        assert_eq!(extracted, vec![
            vec![Some("A".to_string()), Some("B".to_string())],
            vec![Some("C".to_string()), Some("D".to_string())],
        ]);
    }

    #[test]
    fn extract_gives_empty_string_not_none_for_a_populated_grid_slot_with_no_chars() {
        let objs = vec![
            rect(0.0, 0.0, 50.0, 20.0),
            rect(50.0, 0.0, 100.0, 20.0),
            rect(0.0, 20.0, 50.0, 40.0),
            rect(50.0, 20.0, 100.0, 40.0),
        ];
        let tables = find_tables(&objs);
        let extracted = tables[0].extract(&[]);
        assert_eq!(extracted, vec![vec![Some(String::new()), Some(String::new())], vec![Some(String::new()), Some(String::new())]]);
    }
}
