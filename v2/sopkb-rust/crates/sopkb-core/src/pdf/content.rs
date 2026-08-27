//! Content-stream interpretation: PDF operators -> positioned characters,
//! mirroring pdfminer.six's `PDFPageInterpreter` / `PDFTextDevice` / `LTChar`
//! and then pdfplumber's char-dict conversion.
//!
//! This is the piece decision D2 is about. The arithmetic here decides every
//! character's `x0`/`x1`/`top`/`bottom`, and those coordinates are the sole
//! input to pdfplumber's word- and line-breaking, so a small error here shows
//! up as wrong line breaks rather than as an obvious failure.
//!
//! Geometry, verified against pdfplumber 0.11.10 on generated PDFs:
//!   matrix = text_matrix x CTM
//!   glyph bbox in text space = (0, descent + rise, adv, descent + rise + fontsize)
//!   adv  = char_width(code) * fontsize * scaling
//!   top  = page_height - y1,  bottom = page_height - y0,  size = bottom - top
//!
//! Known gaps, all of which degrade to "less text" rather than "wrong text":
//!   - Composite (Type0/CID) fonts use a 2-byte identity decode (see `fonts.rs`).
//!   - Type3 font glyph procedures are not executed, so a Type3 glyph
//!     contributes its advance but no text.
//!   - Clipping paths are ignored (pdfminer ignores them for text too), so text
//!     clipped fully out of view is still extracted.
//!   - Image XObjects are skipped entirely: there is no OCR, exactly as
//!     pdfminer's `do_Do` skips them for text purposes.

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object};
use std::collections::HashMap;

use super::fonts::{resolve, resolve_dict, Font};
use super::graphics::{self, GraphicsObj, PathSeg};
use super::words::PdfChar;

/// A 2x3 PDF transformation matrix `[a b c d e f]`.
type Matrix = [f64; 6];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// pdfminer `mult_matrix(m1, m0)` -- note the argument order: this is
/// `m1 x m0`, expressed in pdfminer's own component form.
fn mult(m1: Matrix, m0: Matrix) -> Matrix {
    let [a1, b1, c1, d1, e1, f1] = m1;
    [
        m0[0] * a1 + m0[2] * b1,
        m0[1] * a1 + m0[3] * b1,
        m0[0] * c1 + m0[2] * d1,
        m0[1] * c1 + m0[3] * d1,
        m0[0] * e1 + m0[2] * f1 + m0[4],
        m0[1] * e1 + m0[3] * f1 + m0[5],
    ]
}

/// pdfminer `translate_matrix`: move the matrix's origin within its own space.
fn translate(m: Matrix, x: f64, y: f64) -> Matrix {
    [m[0], m[1], m[2], m[3], x * m[0] + y * m[2] + m[4], x * m[1] + y * m[3] + m[5]]
}

fn apply_pt(m: Matrix, x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// pdfminer `apply_matrix_rect`: the axis-aligned box that tightly fits the
/// transformed rectangle.
fn apply_rect(m: Matrix, r: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let (x0, y0, x1, y1) = r;
    let corners = [apply_pt(m, x0, y0), apply_pt(m, x1, y0), apply_pt(m, x1, y1), apply_pt(m, x0, y1)];
    let xs: Vec<f64> = corners.iter().map(|c| c.0).collect();
    let ys: Vec<f64> = corners.iter().map(|c| c.1).collect();
    (
        xs.iter().cloned().fold(f64::INFINITY, f64::min),
        ys.iter().cloned().fold(f64::INFINITY, f64::min),
        xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    )
}

/// pdfminer's `PDFTextState`.
#[derive(Debug, Clone)]
struct TextState {
    matrix: Matrix,
    line_matrix: Matrix,
    font: Option<String>,
    fontsize: f64,
    charspace: f64,
    wordspace: f64,
    /// `Tz`, stored as the raw percentage; pdfminer scales by 0.01 at use.
    scaling: f64,
    leading: f64,
    rise: f64,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            matrix: IDENTITY,
            line_matrix: IDENTITY,
            font: None,
            fontsize: 0.0,
            charspace: 0.0,
            wordspace: 0.0,
            scaling: 100.0,
            leading: 0.0,
            rise: 0.0,
        }
    }
}

/// One extracted page.
#[derive(Debug, Clone)]
pub struct Page {
    pub chars: Vec<PdfChar>,
    pub graphics: Vec<GraphicsObj>,
    pub width: f64,
    pub height: f64,
}

/// Extract every page's characters, in document order.
///
/// `doc` must already be decrypted; an encrypted document that could not be
/// opened is the caller's error to report.
pub fn extract_pages(doc: &Document) -> Vec<Page> {
    let mut out = Vec::new();
    let mut doctop = 0.0;
    for (_num, page_id) in doc.get_pages() {
        let page = extract_page(doc, page_id, doctop);
        doctop += page.height;
        out.push(page);
    }
    out
}

fn media_box(doc: &Document, page_id: lopdf::ObjectId) -> (f64, f64, f64, f64) {
    // /MediaBox is inheritable, so walk up /Parent until one is found.
    let mut current = doc.get_dictionary(page_id).ok().cloned();
    let mut depth = 0;
    while let Some(dict) = current {
        if let Ok(obj) = dict.get(b"MediaBox") {
            if let Some(Object::Array(a)) = doc.dereference(obj).ok().map(|(_, o)| o.clone()) {
                if a.len() == 4 {
                    let v: Vec<f64> = a.iter().map(|o| as_num(doc, o).unwrap_or(0.0)).collect();
                    return (v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3]));
                }
            }
        }
        depth += 1;
        if depth > 32 {
            break;
        }
        current = dict.get(b"Parent").ok().and_then(|p| doc.dereference(p).ok()).and_then(|(_, o)| match o {
            Object::Dictionary(d) => Some(d.clone()),
            _ => None,
        });
    }
    (0.0, 0.0, 612.0, 792.0) // US Letter, pdfminer's own fallback
}

fn extract_page(doc: &Document, page_id: lopdf::ObjectId, doctop: f64) -> Page {
    let (mx0, my0, mx1, my1) = media_box(doc, page_id);
    let width = mx1 - mx0;
    let height = my1 - my0;

    let fonts = load_fonts(doc, page_id);
    let content_bytes = doc.get_page_content(page_id);
    let Ok(content) = Content::decode(&content_bytes) else {
        return Page { chars: Vec::new(), graphics: Vec::new(), width, height };
    };

    // pdfminer's base CTM for an unrotated page translates the MediaBox origin
    // to (0, 0); the flip to top-down happens later, in the `top` computation.
    let base_ctm: Matrix = [1.0, 0.0, 0.0, 1.0, -mx0, -my0];

    let page_resources = doc.get_dictionary(page_id).ok().and_then(|d| {
        d.get(b"Resources").ok().and_then(|o| resolve_dict(doc, o))
    });
    let resources = Resources { fonts, dict: page_resources };

    let mut interp = Interpreter {
        doc,
        ctm: base_ctm,
        gstack: Vec::new(),
        ts: TextState::default(),
        chars: Vec::new(),
        graphics: Vec::new(),
        curpath: Vec::new(),
        subpath_start: (0.0, 0.0),
        height,
        doctop,
        visited: std::collections::HashSet::new(),
    };
    interp.run(&content, &resources, 0);
    Page { chars: interp.chars, graphics: interp.graphics, width, height }
}

fn load_fonts(doc: &Document, page_id: lopdf::ObjectId) -> HashMap<String, Font> {
    let mut out = HashMap::new();
    if let Ok(map) = doc.get_page_fonts(page_id) {
        for (name, dict) in map {
            out.insert(String::from_utf8_lossy(&name).to_string(), Font::from_dict(doc, dict));
        }
    }
    out
}

/// Fonts plus the raw resource dictionary, so a nested Form XObject can look up
/// its own `/XObject` and `/Font` entries.
struct Resources {
    fonts: HashMap<String, Font>,
    dict: Option<Dictionary>,
}

fn fonts_from_resources(doc: &Document, res: Option<&Dictionary>) -> HashMap<String, Font> {
    let mut out = HashMap::new();
    if let Some(font_dict) = res.and_then(|d| d.get(b"Font").ok()).and_then(|o| resolve_dict(doc, o)) {
        for (name, obj) in font_dict.iter() {
            if let Some(fd) = resolve_dict(doc, obj) {
                out.insert(String::from_utf8_lossy(name).to_string(), Font::from_dict(doc, &fd));
            }
        }
    }
    out
}

/// pdfminer saves `(ctm, textstate, graphicstate)` on `q` and restores all of it
/// on `Q` -- not just the CTM. Only the two this port models are kept.
struct GraphicsState {
    ctm: Matrix,
    ts: TextState,
}

/// Guards against a Form XObject that (directly or transitively) invokes itself.
/// pdfminer tracks stream ids across nested invocations for the same reason.
const MAX_XOBJECT_DEPTH: usize = 12;

/// One path-construction operator's raw (pre-CTM) endpoint, mirroring
/// pdfminer's `curpath` tuples: `('m'|'l'|'c'|'h', x, y)`. `'c'` here covers
/// pdfminer's `c`/`v`/`y` alike -- only the segment's own endpoint (the final
/// two operands) ever reaches `curpath`; control points are recorded and
/// discarded, exactly as pdfminer's own `paint_path` does by slicing `p[-2:]`.
/// An `'h'` entry's point is the *current subpath's* own starting `m`,
/// substituted here at push time rather than at flush time -- pdfminer does
/// this substitution lazily per `path[0]`, which is only correct because it
/// always operates on one already-`m`-delimited subpath at a time; resolving
/// eagerly here is equivalent and lets `graphics::paint_path`'s own (separate)
/// multi-subpath splitting stay a pure function of already-resolved points.
struct RawPathSeg {
    op: char,
    x: f64,
    y: f64,
}

struct Interpreter<'a> {
    doc: &'a Document,
    ctm: Matrix,
    gstack: Vec<GraphicsState>,
    ts: TextState,
    chars: Vec<PdfChar>,
    graphics: Vec<GraphicsObj>,
    curpath: Vec<RawPathSeg>,
    subpath_start: (f64, f64),
    height: f64,
    doctop: f64,
    visited: std::collections::HashSet<lopdf::ObjectId>,
}

impl Interpreter<'_> {
    fn run(&mut self, content: &Content, res: &Resources, depth: usize) {
        for op in &content.operations {
            let n = |i: usize| -> f64 { op.operands.get(i).and_then(|o| as_num(self.doc, o)).unwrap_or(0.0) };
            match op.operator.as_str() {
                // `q`/`Q` save and restore the TEXT STATE as well as the CTM
                // (pdfminer's get_current_state/set_current_state). Restoring
                // only the CTM leaves a font/size/leading set inside the block
                // leaking out past its `Q`.
                "q" => self.gstack.push(GraphicsState { ctm: self.ctm, ts: self.ts.clone() }),
                "Q" => {
                    if let Some(state) = self.gstack.pop() {
                        self.ctm = state.ctm;
                        self.ts = state.ts;
                    }
                }
                "Do" => self.do_xobject(op.operands.first(), res, depth),
                "cm" => {
                    let m: Matrix = [n(0), n(1), n(2), n(3), n(4), n(5)];
                    self.ctm = mult(m, self.ctm);
                }
                "BT" => {
                    self.ts.matrix = IDENTITY;
                    self.ts.line_matrix = IDENTITY;
                }
                "ET" => {}
                "Tc" => self.ts.charspace = n(0),
                "Tw" => self.ts.wordspace = n(0),
                "Tz" => self.ts.scaling = n(0),
                "TL" => self.ts.leading = n(0),
                "Ts" => self.ts.rise = n(0),
                "Tf" => {
                    self.ts.font = op.operands.first().and_then(|o| match o {
                        Object::Name(nm) => Some(String::from_utf8_lossy(nm).to_string()),
                        _ => None,
                    });
                    self.ts.fontsize = n(1);
                }
                "Td" => self.td(n(0), n(1)),
                "TD" => {
                    self.ts.leading = -n(1);
                    self.td(n(0), n(1));
                }
                "Tm" => {
                    let m: Matrix = [n(0), n(1), n(2), n(3), n(4), n(5)];
                    self.ts.matrix = m;
                    self.ts.line_matrix = m;
                }
                "T*" => self.t_star(),
                "Tj" => {
                    if let Some(Object::String(s, _)) = op.operands.first() {
                        let s = s.clone();
                        self.show(&[TextItem::Bytes(s)], res);
                    }
                }
                "'" => {
                    self.t_star();
                    if let Some(Object::String(s, _)) = op.operands.first() {
                        let s = s.clone();
                        self.show(&[TextItem::Bytes(s)], res);
                    }
                }
                // The `"` operator. Per the PDF spec this must also move to the
                // next line (it is defined as `aw Tw ac Tc T* string Tj`), but
                // pdfminer's `do__w` only does `Tw`, `Tc`, `TJ` -- no `T*`. So
                // pdfminer keeps drawing on the SAME line, continuing from the
                // current pen position. Confirmed against pdfplumber on a
                // generated PDF (see fixtures/harness/diff_pdf_extraction.py's
                // `quote-operators` case), and reproduced deliberately: D2 asks
                // for fidelity to pdfminer's algorithm, quirks included, not to
                // the specification. Adding the `T*` here would put every
                // affected line in the wrong place relative to the reference.
                "\"" => {
                    self.ts.wordspace = n(0);
                    self.ts.charspace = n(1);
                    if let Some(Object::String(s, _)) = op.operands.get(2) {
                        let s = s.clone();
                        self.show(&[TextItem::Bytes(s)], res);
                    }
                }
                "TJ" => {
                    if let Some(Object::Array(arr)) = op.operands.first() {
                        let items: Vec<TextItem> = arr
                            .iter()
                            .filter_map(|o| match o {
                                Object::String(s, _) => Some(TextItem::Bytes(s.clone())),
                                Object::Integer(i) => Some(TextItem::Adjust(*i as f64)),
                                Object::Real(r) => Some(TextItem::Adjust(f64::from(*r))),
                                _ => None,
                            })
                            .collect();
                        self.show(&items, res);
                    }
                }
                // --- path construction: pdfminer's do_m/do_l/do_c/do_v/do_y/
                // do_h/do_re, recording RAW (pre-CTM) endpoints exactly as
                // pdfminer's curpath does -- see `RawPathSeg`'s doc comment.
                "m" => {
                    let (x, y) = (n(0), n(1));
                    self.curpath.push(RawPathSeg { op: 'm', x, y });
                    self.subpath_start = (x, y);
                }
                "l" => self.curpath.push(RawPathSeg { op: 'l', x: n(0), y: n(1) }),
                "c" => self.curpath.push(RawPathSeg { op: 'c', x: n(4), y: n(5) }),
                "v" => self.curpath.push(RawPathSeg { op: 'c', x: n(2), y: n(3) }),
                "y" => self.curpath.push(RawPathSeg { op: 'c', x: n(2), y: n(3) }),
                "h" => {
                    let (x, y) = self.subpath_start;
                    self.curpath.push(RawPathSeg { op: 'h', x, y });
                }
                "re" => {
                    let (x, y, w, h) = (n(0), n(1), n(2), n(3));
                    self.curpath.push(RawPathSeg { op: 'm', x, y });
                    self.curpath.push(RawPathSeg { op: 'l', x: x + w, y });
                    self.curpath.push(RawPathSeg { op: 'l', x: x + w, y: y + h });
                    self.curpath.push(RawPathSeg { op: 'l', x, y: y + h });
                    self.curpath.push(RawPathSeg { op: 'h', x, y });
                    self.subpath_start = (x, y);
                }
                // --- path painting: pdfminer's do_S/do_s/do_f/do_F/do_f_a/
                // do_B/do_B_a/do_b/do_b_a/do_n. `s`/`b`/`b_a` close the path
                // (append an "h") before painting, same as pdfminer's own
                // `do_h(); do_S()`-style delegation. `F` (obsolete fill) is a
                // genuine no-op in pdfminer -- it neither paints nor clears
                // curpath -- so it is deliberately absent below, exactly
                // mirroring `do_F`'s empty body. `W`/`W*` (clipping) are also
                // absent: pdfminer's do_W/do_W_a don't touch curpath either,
                // so the following paint/`n` operator behaves as if they were
                // never there, matching real behavior for both "W n"
                // (clip-only, paints nothing) and "W f" (clips AND fills).
                "S" => self.flush_path(true, false),
                "s" => {
                    let (x, y) = self.subpath_start;
                    self.curpath.push(RawPathSeg { op: 'h', x, y });
                    self.flush_path(true, false);
                }
                "f" | "f*" => self.flush_path(false, true),
                "B" => self.flush_path(true, true),
                "B*" => self.flush_path(true, true),
                "b" => {
                    let (x, y) = self.subpath_start;
                    self.curpath.push(RawPathSeg { op: 'h', x, y });
                    self.flush_path(true, true);
                }
                "b*" => {
                    let (x, y) = self.subpath_start;
                    self.curpath.push(RawPathSeg { op: 'h', x, y });
                    self.flush_path(true, true);
                }
                "n" => self.curpath.clear(),
                _ => {}
            }
        }
    }

    /// Transform `curpath` by the CTM **active right now** (matching
    /// pdfminer: all painting -- even of segments recorded under an earlier
    /// `cm` -- uses `self.ctm` at paint time, not at construction time; see
    /// `converter.py::paint_path`'s `apply_matrix_pt(self.ctm, pt)`), flip to
    /// top-down page coordinates the same way `render_char` does for text
    /// (`top = height - y`, matching pdfplumber's own `point2coord`), hand
    /// the resolved segments to `graphics::paint_path`, and clear curpath.
    fn flush_path(&mut self, stroke: bool, fill: bool) {
        if self.curpath.is_empty() {
            return;
        }
        let segs: Vec<PathSeg> = self
            .curpath
            .iter()
            .map(|s| {
                let (tx, ty) = apply_pt(self.ctm, s.x, s.y);
                PathSeg { op: s.op, pt: (tx, self.height - ty) }
            })
            .collect();
        self.graphics.extend(graphics::paint_path(&segs, stroke, fill));
        self.curpath.clear();
    }

    /// `Do`: invoke a named XObject. Only Form XObjects carrying a `/BBox` are
    /// executed (pdfminer's `do_Do` requires both); an Image XObject has no text
    /// and there is no OCR, so it is skipped exactly as pdfminer skips it here.
    ///
    /// The form runs with `ctm = mult(form_matrix, self.ctm)`, its OWN
    /// `/Resources` if present (falling back to the invoking resources, per PDF
    /// 1.7 §4.9.1 for pre-1.2 files), and a FRESH text state -- pdfminer builds
    /// a sub-interpreter, so nothing about the caller's text state leaks in.
    fn do_xobject(&mut self, operand: Option<&Object>, res: &Resources, depth: usize) {
        if depth >= MAX_XOBJECT_DEPTH {
            return;
        }
        let Some(Object::Name(raw)) = operand else { return };
        let name = String::from_utf8_lossy(raw).to_string();

        let Some(xobjects) = res.dict.as_ref().and_then(|d| d.get(b"XObject").ok()).and_then(|o| resolve_dict(self.doc, o))
        else {
            return;
        };
        let Ok(entry) = xobjects.get(name.as_bytes()) else { return };

        // Resolve to the stream, keeping its ObjectId for the cycle guard.
        let obj_id = match entry {
            Object::Reference(id) => Some(*id),
            _ => None,
        };
        let Some((_, resolved)) = self.doc.dereference(entry).ok() else { return };
        let Object::Stream(stream) = resolved else { return };

        let subtype = stream.dict.get(b"Subtype").ok().and_then(|o| resolve(self.doc, o)).and_then(|o| match o {
            Object::Name(n) => Some(String::from_utf8_lossy(&n).to_string()),
            _ => None,
        });
        if subtype.as_deref() != Some("Form") {
            return;
        }
        if stream.dict.get(b"BBox").is_err() {
            return;
        }
        if let Some(id) = obj_id {
            if !self.visited.insert(id) {
                return; // already on the invocation path -- a cycle
            }
        }

        let matrix: Matrix = match stream.dict.get(b"Matrix").ok().and_then(|o| resolve(self.doc, o)) {
            Some(Object::Array(a)) if a.len() == 6 => {
                let v: Vec<f64> = a.iter().map(|o| as_num(self.doc, o).unwrap_or(0.0)).collect();
                [v[0], v[1], v[2], v[3], v[4], v[5]]
            }
            _ => IDENTITY,
        };

        let own_res = stream.dict.get(b"Resources").ok().and_then(|o| resolve_dict(self.doc, o));
        let nested = match own_res {
            Some(d) => Resources { fonts: fonts_from_resources(self.doc, Some(&d)), dict: Some(d) },
            None => Resources {
                fonts: fonts_from_resources(self.doc, res.dict.as_ref()),
                dict: res.dict.clone(),
            },
        };

        let data = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
        if let Ok(inner) = Content::decode(&data) {
            let saved_ctm = self.ctm;
            let saved_ts = std::mem::take(&mut self.ts);
            let saved_gstack = std::mem::take(&mut self.gstack);
            self.ctm = mult(matrix, saved_ctm);
            self.run(&inner, &nested, depth + 1);
            self.ctm = saved_ctm;
            self.ts = saved_ts;
            self.gstack = saved_gstack;
        }
        if let Some(id) = obj_id {
            self.visited.remove(&id);
        }
    }

    /// `Td`: `Tlm = translate(tx, ty) x Tlm`, and `Tm` is reset to it.
    fn td(&mut self, tx: f64, ty: f64) {
        let t: Matrix = [1.0, 0.0, 0.0, 1.0, tx, ty];
        self.ts.line_matrix = mult(t, self.ts.line_matrix);
        self.ts.matrix = self.ts.line_matrix;
    }

    fn t_star(&mut self) {
        let leading = self.ts.leading;
        self.td(0.0, -leading);
    }

    /// pdfminer `render_string_horizontal`.
    fn show(&mut self, seq: &[TextItem], res: &Resources) {
        let Some(font_name) = self.ts.font.clone() else { return };
        let Some(font) = res.fonts.get(&font_name) else { return };

        let matrix = mult(self.ts.matrix, self.ctm);
        let fontsize = self.ts.fontsize;
        let scaling = self.ts.scaling * 0.01;
        let charspace = self.ts.charspace * scaling;
        // "if font.is_multibyte(): wordspace = 0"
        let wordspace = if font.is_multibyte { 0.0 } else { self.ts.wordspace * scaling };
        let rise = self.ts.rise;
        let dxscale = 0.001 * fontsize * scaling;

        // pdfminer tracks the pen position within the text matrix's own space,
        // starting at (0, 0) for each show op because `matrix` already carries
        // the origin; `translate_matrix` then moves the glyph into place.
        let mut pen_x = 0.0;
        let pen_y = 0.0;
        let mut needcharspace = false;

        for item in seq {
            match item {
                TextItem::Adjust(v) => {
                    pen_x -= v * dxscale;
                    needcharspace = true;
                }
                TextItem::Bytes(bytes) => {
                    for code in font.decode(bytes) {
                        if needcharspace {
                            pen_x += charspace;
                        }
                        let m = translate(matrix, pen_x, pen_y);
                        pen_x += self.render_char(m, font, fontsize, scaling, rise, code);
                        if code == 32 && wordspace != 0.0 && !font.is_multibyte {
                            pen_x += wordspace;
                        }
                        needcharspace = true;
                    }
                }
            }
        }

        // Advance the text matrix by the total displacement, so a following
        // show op on the same line starts in the right place.
        self.ts.matrix = translate(self.ts.matrix, pen_x, pen_y);
    }

    /// pdfminer `render_char` + `LTChar.__init__`, then pdfplumber's char dict.
    fn render_char(&mut self, matrix: Matrix, font: &Font, fontsize: f64, scaling: f64, rise: f64, code: u32) -> f64 {
        let textwidth = font.char_width(code);
        let adv = textwidth * fontsize * scaling;

        // pdfminer raises PDFUnicodeNotDefined and the device emits nothing
        // usable; pdfplumber surfaces such chars with their raw text. Falling
        // back to the code point keeps the glyph's advance (and therefore every
        // later glyph's position) correct even when the text is unknown.
        let text = font.to_unichr(code).unwrap_or_else(|| char::from_u32(code).map(|c| c.to_string()).unwrap_or_default());

        let descent = font.descent() * fontsize;
        let bbox = (0.0, descent + rise, adv, descent + rise + fontsize);
        let [a, b, c, d, _, _] = matrix;
        let upright = a * d * scaling > 0.0 && b * c <= 0.0;
        let (mut x0, mut y0, mut x1, mut y1) = apply_rect(matrix, bbox);
        if x1 < x0 {
            std::mem::swap(&mut x0, &mut x1);
        }
        if y1 < y0 {
            std::mem::swap(&mut y0, &mut y1);
        }

        let top = self.height - y1;
        let bottom = self.height - y0;
        self.chars.push(PdfChar {
            text,
            x0,
            x1,
            top,
            bottom,
            doctop: self.doctop + top,
            upright,
            // LTChar.size is the device-space height for horizontal writing.
            size: bottom - top,
            fontname: font.fontname.clone(),
        });
        adv
    }
}

enum TextItem {
    Bytes(Vec<u8>),
    Adjust(f64),
}

fn as_num(doc: &Document, obj: &Object) -> Option<f64> {
    match doc.dereference(obj).ok().map(|(_, o)| o.clone())? {
        Object::Integer(i) => Some(i as f64),
        Object::Real(r) => Some(f64::from(r)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mult_matches_pdfminer_component_order() {
        // pdfminer: mult_matrix(m1, m0) applies m1 then m0.
        let scale: Matrix = [2.0, 0.0, 0.0, 2.0, 0.0, 0.0];
        let shift: Matrix = [1.0, 0.0, 0.0, 1.0, 10.0, 20.0];
        // Scale then shift: a point at (1,1) -> (2,2) -> (12,22).
        let m = mult(scale, shift);
        assert_eq!(apply_pt(m, 1.0, 1.0), (12.0, 22.0));
        // Shift then scale: (1,1) -> (11,21) -> (22,42).
        let m = mult(shift, scale);
        assert_eq!(apply_pt(m, 1.0, 1.0), (22.0, 42.0));
    }

    #[test]
    fn translate_moves_origin_within_own_space() {
        let m: Matrix = [2.0, 0.0, 0.0, 2.0, 0.0, 0.0];
        // Translating by (3, 0) in the matrix's own space shifts the origin by
        // 6 device units, because the matrix scales by 2.
        assert_eq!(translate(m, 3.0, 0.0), [2.0, 0.0, 0.0, 2.0, 6.0, 0.0]);
    }

    #[test]
    fn apply_rect_fits_a_rotated_box() {
        // 90-degree rotation: the unit square maps to x in [-1,0], y in [0,1].
        let rot: Matrix = [0.0, 1.0, -1.0, 0.0, 0.0, 0.0];
        let (x0, y0, x1, y1) = apply_rect(rot, (0.0, 0.0, 1.0, 1.0));
        assert!((x0 - -1.0).abs() < 1e-9 && (y0 - 0.0).abs() < 1e-9);
        assert!((x1 - 0.0).abs() < 1e-9 && (y1 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn upright_flag_follows_pdfminers_formula() {
        // Identity is upright; a 90-degree rotation is not.
        let up: Matrix = IDENTITY;
        assert!(up[0] * up[3] * 1.0 > 0.0 && up[1] * up[2] <= 0.0);
        let rot: Matrix = [0.0, 1.0, -1.0, 0.0, 0.0, 0.0];
        assert!(!(rot[0] * rot[3] * 1.0 > 0.0 && rot[1] * rot[2] <= 0.0));
    }
}
