//! Vector-graphics objects (rects/lines/curves), extracted from a PDF content
//! stream's path-construction/painting operators, needed to find tables the
//! same way pdfplumber does: `page.find_tables()`'s default "lines" strategy
//! reads actual drawn geometry, not text layout. Nothing here reads color or
//! line width -- pdfplumber's own table-finding edges (`rect_to_edges`,
//! `line_to_edge`, `curve_to_edges` in its `utils/geometry.py`) never use
//! them either.
//!
//! Path-to-shape classification mirrors pdfminer.six's `PDFConverter.paint_path`
//! (`pdfminer/converter.py`) exactly, verified by reading pdfminer.six 20260... in
//! a local vendored install (pinned by oss-launch's own `pdfplumber>=0.11`
//! dependency): a subpath shaped `mlh`/`ml` (built from one `moveto` + one
//! `lineto`, painted) is a [`GraphicsObj::Line`]; one shaped `mlllh`/`mllll`
//! (`moveto` + 3-4 more points back to the start) that is both closed AND
//! axis-aligned is a [`GraphicsObj::Rect`]; everything else -- including any
//! path containing a real Bezier `c`/`v`/`y` segment -- is a
//! [`GraphicsObj::Curve`]. A path is only classified when it is actually
//! PAINTED (stroked and/or filled); a clip-only path (ending in a bare `n`
//! after `W`/`W*`) produces nothing, matching pdfminer's own behavior of
//! never invoking `paint_path` for a pure clip.

/// One path-construction operator's contribution to shape classification,
/// tracked as a single letter (`m`/`l`/`c`/`h`) plus that operator's own
/// endpoint in already-CTM-transformed, top-down page coordinates (`x`,
/// `top`-style `y`, i.e. `height - y`). `v`/`y` (the two-control-point
/// Bezier variants) collapse to `'c'` here -- pdfminer's own shape string
/// only distinguishes "a point was added" (`l`/`c`/`v`/`y` all count as one
/// letter each for `mlllh`-style matching) from `m`/`h`, never `c` from `v`
/// from `y`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathSeg {
    pub op: char,
    pub pt: (f64, f64),
}

/// One classified vector-graphics object, in the same top-down `x0/x1/top/
/// bottom` coordinate convention as [`super::words::PdfChar`].
#[derive(Debug, Clone, PartialEq)]
pub enum GraphicsObj {
    Rect { x0: f64, top: f64, x1: f64, bottom: f64 },
    Line { x0: f64, top: f64, x1: f64, bottom: f64 },
    /// `pts`: every segment's endpoint, in order, `(x, top-down-y)` --
    /// `curve_to_edges` pairs consecutive points, so order matters.
    Curve { x0: f64, top: f64, x1: f64, bottom: f64, pts: Vec<(f64, f64)> },
}

impl GraphicsObj {
    pub fn bbox(&self) -> (f64, f64, f64, f64) {
        match self {
            GraphicsObj::Rect { x0, top, x1, bottom } => (*x0, *top, *x1, *bottom),
            GraphicsObj::Line { x0, top, x1, bottom } => (*x0, *top, *x1, *bottom),
            GraphicsObj::Curve { x0, top, x1, bottom, .. } => (*x0, *top, *x1, *bottom),
        }
    }
}

fn bbox_of(pts: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let xs = pts.iter().map(|p| p.0);
    let ys = pts.iter().map(|p| p.1);
    (
        xs.clone().fold(f64::INFINITY, f64::min),
        ys.clone().fold(f64::INFINITY, f64::min),
        xs.fold(f64::NEG_INFINITY, f64::max),
        ys.fold(f64::NEG_INFINITY, f64::max),
    )
}

/// `PDFConverter.paint_path`: classify one full path-painting invocation
/// (which may contain multiple `m`-started subpaths) into zero or more
/// [`GraphicsObj`]s. Returns nothing for a path that isn't actually painted
/// (`stroke == false && fill == false`, i.e. a clip-only path) or one that
/// doesn't start with `m` (per pdfminer, not a valid path at all).
pub fn paint_path(path: &[PathSeg], stroke: bool, fill: bool) -> Vec<GraphicsObj> {
    if !stroke && !fill {
        return Vec::new();
    }
    if path.first().map(|s| s.op) != Some('m') {
        return Vec::new();
    }

    // Split on each 'm' into independent subpaths -- pdfminer recurses when a
    // single paint invocation's path contains more than one 'm'.
    let mut subpaths: Vec<&[PathSeg]> = Vec::new();
    let mut start = 0usize;
    for i in 1..path.len() {
        if path[i].op == 'm' {
            subpaths.push(&path[start..i]);
            start = i;
        }
    }
    subpaths.push(&path[start..]);

    subpaths.into_iter().filter_map(classify_subpath).collect()
}

fn classify_subpath(seg: &[PathSeg]) -> Option<GraphicsObj> {
    let mut shape: String = seg.iter().map(|s| s.op).collect();
    let mut pts: Vec<(f64, f64)> = seg.iter().map(|s| s.pt).collect();

    // Drop a redundant final "l" on a path closed with "h": `pdfminer`'s own
    // fix for hand-drawn rectangles that both lineto back to their start AND
    // then issue a closepath.
    if shape.len() > 3 && shape.ends_with("lh") && pts.len() >= 2 && pts[pts.len() - 2] == pts[0] {
        shape.truncate(shape.len() - 2);
        shape.push('h');
        pts.pop();
    }

    if shape == "mlh" || shape == "ml" {
        let (x0, top, x1, bottom) = bbox_of(&pts[..2]);
        return Some(GraphicsObj::Line { x0, top, x1, bottom });
    }

    if (shape == "mlllh" || shape == "mllll") && pts.len() >= 5 {
        let (p0, p1, p2, p3) = (pts[0], pts[1], pts[2], pts[3]);
        let is_closed_loop = pts[0] == pts[4];
        let has_square_coordinates = (p0.0 == p1.0 && p1.1 == p2.1 && p2.0 == p3.0 && p3.1 == p0.1)
            || (p0.1 == p1.1 && p1.0 == p2.0 && p2.1 == p3.1 && p3.0 == p0.0);
        if is_closed_loop && has_square_coordinates {
            let (x0, top, x1, bottom) = bbox_of(&pts[..4]);
            return Some(GraphicsObj::Rect { x0, top, x1, bottom });
        }
        let (x0, top, x1, bottom) = bbox_of(&pts);
        return Some(GraphicsObj::Curve { x0, top, x1, bottom, pts });
    }

    if pts.len() < 2 {
        return None;
    }
    let (x0, top, x1, bottom) = bbox_of(&pts);
    Some(GraphicsObj::Curve { x0, top, x1, bottom, pts })
}

/// Expands a PDF `re x y w h` operator into its pdfminer-equivalent `m l l l
/// h` point sequence, already in the caller's coordinate convention (the
/// caller applies the CTM and the top-down flip to each corner before
/// calling this -- see `content.rs`'s `re` handling).
pub fn rect_op_corners(x0: f64, y0: f64, x1: f64, y1: f64) -> [(f64, f64); 4] {
    [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(op: char, x: f64, y: f64) -> PathSeg {
        PathSeg { op, pt: (x, y) }
    }

    #[test]
    fn two_point_stroked_path_is_a_line() {
        let path = [seg('m', 10.0, 20.0), seg('l', 30.0, 20.0)];
        let objs = paint_path(&path, true, false);
        assert_eq!(objs, vec![GraphicsObj::Line { x0: 10.0, top: 20.0, x1: 30.0, bottom: 20.0 }]);
    }

    #[test]
    fn re_expansion_shape_is_a_rect() {
        let corners = rect_op_corners(10.0, 20.0, 110.0, 70.0);
        let path = [
            seg('m', corners[0].0, corners[0].1),
            seg('l', corners[1].0, corners[1].1),
            seg('l', corners[2].0, corners[2].1),
            seg('l', corners[3].0, corners[3].1),
            seg('h', corners[0].0, corners[0].1),
        ];
        let objs = paint_path(&path, true, true);
        assert_eq!(objs, vec![GraphicsObj::Rect { x0: 10.0, top: 20.0, x1: 110.0, bottom: 70.0 }]);
    }

    #[test]
    fn explicit_four_lineto_rect_with_redundant_closing_h_is_still_a_rect() {
        // m l l l l h -- four explicit linetos back to the start, PLUS a
        // redundant closepath. Must collapse to the same 5-point rect pdfminer
        // recognizes, not fall through to Curve.
        let path = [
            seg('m', 0.0, 0.0),
            seg('l', 100.0, 0.0),
            seg('l', 100.0, 50.0),
            seg('l', 0.0, 50.0),
            seg('l', 0.0, 0.0),
            seg('h', 0.0, 0.0),
        ];
        let objs = paint_path(&path, true, false);
        assert_eq!(objs, vec![GraphicsObj::Rect { x0: 0.0, top: 0.0, x1: 100.0, bottom: 50.0 }]);
    }

    #[test]
    fn non_axis_aligned_quadrilateral_is_a_curve_not_a_rect() {
        let path = [seg('m', 0.0, 0.0), seg('l', 100.0, 10.0), seg('l', 90.0, 60.0), seg('l', 5.0, 55.0), seg('h', 0.0, 0.0)];
        let objs = paint_path(&path, true, false);
        assert!(matches!(objs[0], GraphicsObj::Curve { .. }), "{objs:?}");
    }

    #[test]
    fn clip_only_path_produces_nothing() {
        let path = [seg('m', 0.0, 0.0), seg('l', 100.0, 0.0)];
        assert!(paint_path(&path, false, false).is_empty());
    }

    #[test]
    fn path_not_starting_with_moveto_produces_nothing() {
        let path = [seg('l', 0.0, 0.0), seg('l', 100.0, 0.0)];
        assert!(paint_path(&path, true, false).is_empty());
    }

    #[test]
    fn multiple_subpaths_in_one_paint_invocation_are_classified_independently() {
        // Two separate 2-point lines drawn by one stroke operator.
        let path = [seg('m', 0.0, 0.0), seg('l', 10.0, 0.0), seg('m', 0.0, 20.0), seg('l', 10.0, 20.0)];
        let objs = paint_path(&path, true, false);
        assert_eq!(objs.len(), 2);
        assert!(objs.iter().all(|o| matches!(o, GraphicsObj::Line { .. })));
    }

    #[test]
    fn bezier_curveto_is_never_a_rect_even_with_four_segments() {
        let path = [seg('m', 0.0, 0.0), seg('c', 10.0, 10.0), seg('l', 20.0, 0.0), seg('l', 0.0, 0.0), seg('h', 0.0, 0.0)];
        let objs = paint_path(&path, true, false);
        assert!(matches!(objs[0], GraphicsObj::Curve { .. }));
    }
}
