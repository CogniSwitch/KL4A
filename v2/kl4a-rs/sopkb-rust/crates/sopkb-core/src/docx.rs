//! DOCX normalization (docs/port/CATCHUP_PLAN.md decision D3, docs/port/port-mapping-a-core-data.md
//! `FUNCTION normalize_docx` + `heading_level_for_style` + `markdown_table`). Feature-gated behind
//! `docx` (see `Cargo.toml`).
//!
//! A DOCX file is a zip archive; the body lives in `word/document.xml`. Rather than depend on a
//! pre-built Rust docx crate (which would have its own, different opinions about traversal order and
//! text joining -- decision D3), this hand-parses the OOXML directly with `zip` + `quick-xml`,
//! replicating python-docx 1.2.0's specific behavior (verified empirically against a throwaway venv,
//! not just read from its source -- see the deviation notes below for two places where reading the
//! source alone would have been misleading).
//!
//! Deviations from `port-mapping-a-core-data.md`'s pseudocode, found while verifying against actual
//! python-docx 1.2.0 behavior (not just the pseudocode) per this workstream's brief:
//!
//! 1. **Style names go through a "BabelFish" UI-name translation, not a raw XML read.** The
//!    pseudocode's `style_name = paragraph.style.name` undersells this: the raw `<w:name w:val="..."/>`
//!    stored in `word/styles.xml` for a built-in heading style is lowercase (`"heading 1"`), and
//!    `Style.name` runs it through `docx.styles.BabelFish.internal2ui`, an explicit 12-entry alias
//!    table (Caption/Footer/Header/Heading 1..9) that capitalizes exactly those names and passes
//!    everything else through unchanged. Skipping this step would mean *no* real DOCX heading is ever
//!    detected, since `heading_level_for_style`'s regex is case-sensitive. Replicated in
//!    `babelfish_internal2ui` below.
//! 2. **A cell's `.text` does NOT include nested-table text**, contrary to the pseudocode's
//!    parenthetical ("their outer cell.text does include the nested text, via its paragraphs") --
//!    this was flagged in the task brief as needing verification, and empirical testing
//!    (`_Cell.text` = `"\n".join(p.text for p in self.paragraphs)`, and `self.paragraphs` only
//!    returns *direct*-child `<w:p>` elements, never descending into a nested `<w:tbl>`) confirms the
//!    pseudocode's comment is wrong for python-docx 1.2.0. A cell containing only a nested table (no
//!    paragraph text of its own) contributes only the empty/near-empty text of its own direct-child
//!    paragraphs (Word always requires at least one trailing `<w:p>` after a nested table, so this is
//!    usually `""` or `"\n"`, never the nested table's content). Implemented by only reading a `<w:tc>`
//!    element's *direct*-child `<w:p>` elements for its own text.
//!
//! Vertical cell merges (`w:vMerge`) are also resolved (walking up through `continue` cells to the
//! `restart` cell, matching python-docx's `_Row.cells`/`CT_Tc._tc_above`/`grid_offset` algorithm)
//! even though the required fixtures only exercise horizontal merges -- once grid-span duplication
//! was implemented, getting vertical-merge resolution right too was a small marginal cost for
//! genuine fidelity to "current oss-launch behavior" (D1), rather than a special case to skip.

use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;

/// `docx.styles.BabelFish.style_aliases` (python-docx `docx/styles/__init__.py`): translates the raw
/// internal style name stored in `styles.xml` to the UI name `Style.name` returns. Anything not in
/// this table passes through unchanged -- in particular, localized names like `"Titre 1"` (French) or
/// `"Überschrift 1"` (German) are untouched, which is *why* they fail `heading_level_for_style` (they
/// were never `"heading 1"` internally to begin with).
const BABELFISH_ALIASES: &[(&str, &str)] = &[
    ("caption", "Caption"),
    ("footer", "Footer"),
    ("header", "Header"),
    ("heading 1", "Heading 1"),
    ("heading 2", "Heading 2"),
    ("heading 3", "Heading 3"),
    ("heading 4", "Heading 4"),
    ("heading 5", "Heading 5"),
    ("heading 6", "Heading 6"),
    ("heading 7", "Heading 7"),
    ("heading 8", "Heading 8"),
    ("heading 9", "Heading 9"),
];

fn babelfish_internal2ui(internal_name: &str) -> String {
    BABELFISH_ALIASES
        .iter()
        .find(|(k, _)| *k == internal_name)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| internal_name.to_string())
}

/// `re.match(r"Heading\s+([1-6])$", style_name)`, hand-rolled the same way `normalize.rs::find_headings`
/// hand-rolls the ATX heading regex (no runtime `regex` dependency in this crate -- see Cargo.toml).
/// Anchored at both start (`Heading` must be the literal first 7 bytes, case-sensitive) and end
/// (nothing after the digit, except Python's `$` also allowing exactly one trailing `\n`).
pub(crate) fn heading_level_for_style(style_name: &str) -> Option<u8> {
    let rest = style_name.strip_prefix("Heading")?;
    let chars: Vec<char> = rest.chars().collect();
    let mut i = 0;
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i == 0 || i >= chars.len() {
        return None; // `\s+` requires at least one whitespace char, then a digit must follow
    }
    let digit = chars[i].to_digit(10)? as u8;
    if !(1..=6).contains(&digit) {
        return None;
    }
    match &chars[i + 1..] {
        [] => Some(digit),
        ['\n'] => Some(digit), // Python `$` also matches immediately before a single trailing "\n"
        _ => None,
    }
}

/// `" ".join(text.split())`: split on any whitespace run, drop empties, rejoin with single spaces.
/// Leading/trailing whitespace disappears because `str.split()` never yields leading/trailing empty
/// strings the way `str.split(" ")` would.
fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `FUNCTION markdown_table` (port-mapping-a-core-data.md). No existing helper of this name was found
/// elsewhere in `sopkb-core`/`sopkb-fmt` (checked before writing this). The first row is always the
/// header, even for headerless tables; cell text is not Markdown-escaped (a `|` inside a cell breaks
/// the table, matching Python).
pub(crate) fn markdown_table(rows: &[Vec<String>]) -> String {
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let padded: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            let mut r = row.clone();
            r.resize(width, String::new());
            r
        })
        .collect();
    let header = format!("| {} |", padded[0].join(" | "));
    let separator = format!("| {} |", vec!["---"; width].join(" | "));
    let mut lines = vec![header, separator];
    for row in &padded[1..] {
        lines.push(format!("| {} |", row.join(" | ")));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------------------------
// Minimal generic XML tree (not OOXML-specific) -- built once per part (`document.xml`/`styles.xml`)
// so the traversal logic below can do simple recursive-descent/child-lookup instead of juggling a
// manual depth/stack machine over quick-xml's raw event stream.
// ---------------------------------------------------------------------------------------------

struct XmlElement {
    /// Local name only (namespace prefix stripped) -- e.g. "p", "tbl", "tcPr". OOXML consistently
    /// uses the "w:" prefix for the WordprocessingML namespace in both real Word output and
    /// python-docx's own output, so matching on local name only (ignoring the prefix entirely,
    /// rather than resolving namespace URIs) is robust in practice and much simpler.
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<XmlNode>,
}

enum XmlNode {
    Element(XmlElement),
    Text(String),
}

impl XmlElement {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a XmlElement> {
        self.children.iter().filter_map(move |c| match c {
            XmlNode::Element(el) if el.name == name => Some(el),
            _ => None,
        })
    }

    fn child_named<'a>(&'a self, name: &'a str) -> Option<&'a XmlElement> {
        self.children_named(name).next()
    }

    /// Concatenation of direct-child text nodes only (not descendants) -- used for leaf text
    /// elements like `<w:t>` where the text is always a direct child, never nested.
    fn direct_text(&self) -> String {
        let mut out = String::new();
        for c in &self.children {
            if let XmlNode::Text(t) = c {
                out.push_str(t);
            }
        }
        out
    }
}

fn local_name(qname: QName) -> String {
    String::from_utf8_lossy(qname.local_name().as_ref()).into_owned()
}

fn parse_xml_tree(xml: &str) -> Result<XmlElement, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false); // preserve whitespace verbatim (xml:space="preserve" text)
    let mut stack: Vec<XmlElement> = Vec::new();
    let mut root: Option<XmlElement> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let mut attrs = Vec::new();
                for a in e.attributes().flatten() {
                    let key = local_name(a.key);
                    if let Ok(val) = a.normalized_value(quick_xml::XmlVersion::Implicit1_0) {
                        attrs.push((key, val.into_owned()));
                    }
                }
                stack.push(XmlElement { name: local_name(e.name()), attrs, children: Vec::new() });
            }
            Ok(Event::Empty(e)) => {
                let mut attrs = Vec::new();
                for a in e.attributes().flatten() {
                    let key = local_name(a.key);
                    if let Ok(val) = a.normalized_value(quick_xml::XmlVersion::Implicit1_0) {
                        attrs.push((key, val.into_owned()));
                    }
                }
                let el = XmlElement { name: local_name(e.name()), attrs, children: Vec::new() };
                append_node(&mut stack, &mut root, XmlNode::Element(el));
            }
            Ok(Event::End(_)) => {
                let el = stack.pop().ok_or_else(|| "unbalanced XML: unexpected close tag".to_string())?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(XmlNode::Element(el));
                } else {
                    root = Some(el);
                }
            }
            Ok(Event::Text(t)) => {
                if let Ok(decoded) = t.decode() {
                    if let Ok(text) = quick_xml::escape::unescape(&decoded) {
                        if let Some(top) = stack.last_mut() {
                            top.children.push(XmlNode::Text(text.into_owned()));
                        }
                    }
                }
            }
            Ok(Event::CData(t)) => {
                let text = String::from_utf8_lossy(t.as_ref()).into_owned();
                if let Some(top) = stack.last_mut() {
                    top.children.push(XmlNode::Text(text));
                }
            }
            Ok(_) => {} // comments, processing instructions, doctype -- irrelevant here
            Err(e) => return Err(format!("malformed XML: {e}")),
        }
    }

    root.ok_or_else(|| "malformed XML: no root element".to_string())
}

fn append_node(stack: &mut [XmlElement], root: &mut Option<XmlElement>, node: XmlNode) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else if let XmlNode::Element(el) = node {
        *root = Some(el);
    }
}

// ---------------------------------------------------------------------------------------------
// Style table (word/styles.xml) -- needed to translate a paragraph's `w:pStyle` id into the display
// name `heading_level_for_style` matches against.
// ---------------------------------------------------------------------------------------------

struct StyleTable {
    /// styleId -> UI display name (already BabelFish-translated), paragraph styles only. Matches
    /// python-docx's `get_by_id(style_id, PARAGRAPH)`, which treats a styleId of the wrong type as
    /// "not found" and falls back to the default paragraph style.
    by_id: std::collections::HashMap<String, String>,
    /// The document's default paragraph style name (UI, translated), if `styles.xml` declares one
    /// (`<w:style w:type="paragraph" w:default="1">`).
    default_paragraph_name: Option<String>,
}

impl StyleTable {
    fn empty() -> Self {
        StyleTable { by_id: std::collections::HashMap::new(), default_paragraph_name: None }
    }

    /// `paragraph.style.name if paragraph.style is not None else ""`, given a `w:pStyle/@w:val`
    /// (or `None` if the paragraph has no `w:pPr/w:pStyle` at all).
    fn resolve(&self, style_id: Option<&str>) -> String {
        if let Some(id) = style_id {
            if let Some(name) = self.by_id.get(id) {
                return name.clone();
            }
        }
        self.default_paragraph_name.clone().unwrap_or_default()
    }
}

fn build_style_table(styles_xml: Option<&str>) -> Result<StyleTable, String> {
    let Some(xml) = styles_xml else {
        return Ok(StyleTable::empty());
    };
    let root = parse_xml_tree(xml)?;
    let mut by_id = std::collections::HashMap::new();
    let mut default_paragraph_name = None;

    for style_el in root.children_named("style") {
        if style_el.attr("type") != Some("paragraph") {
            continue;
        }
        let Some(style_id) = style_el.attr("styleId") else { continue };
        let raw_name = style_el.child_named("name").and_then(|n| n.attr("val")).unwrap_or("");
        let ui_name = babelfish_internal2ui(raw_name);

        let is_default = matches!(style_el.attr("default"), Some("1") | Some("true"));
        if is_default {
            default_paragraph_name = Some(ui_name.clone());
        }
        by_id.insert(style_id.to_string(), ui_name);
    }

    Ok(StyleTable { by_id, default_paragraph_name })
}

// ---------------------------------------------------------------------------------------------
// Run / paragraph / hyperlink text extraction (`Run.text`, `CT_P.text`, `CT_Hyperlink.text`).
// ---------------------------------------------------------------------------------------------

/// `CT_R.text`: concatenation, in document order, of the text-equivalent of each direct-child
/// `w:t` / `w:tab` / `w:br` / `w:cr` / `w:noBreakHyphen` / `w:ptab` element. Other run children
/// (`w:rPr`, `w:drawing`, footnote/comment references, ...) contribute nothing.
fn run_text(run_el: &XmlElement) -> String {
    let mut out = String::new();
    for child in &run_el.children {
        let XmlNode::Element(el) = child else { continue };
        match el.name.as_str() {
            "t" => out.push_str(&el.direct_text()),
            "tab" | "ptab" => out.push('\t'),
            "br" => {
                // CT_Br.__str__: "\n" for type == "textWrapping" (the default when @type is
                // absent), "" for "page"/"column" breaks.
                let br_type = el.attr("type").unwrap_or("textWrapping");
                if br_type == "textWrapping" {
                    out.push('\n');
                }
            }
            "cr" => out.push('\n'),
            "noBreakHyphen" => out.push('-'),
            _ => {}
        }
    }
    out
}

/// `CT_P.text`: `"".join(e.text for e in self.xpath("w:r | w:hyperlink"))` -- direct-child runs and
/// hyperlinks only, in document order.
fn paragraph_text(p_el: &XmlElement) -> String {
    let mut out = String::new();
    for child in &p_el.children {
        let XmlNode::Element(el) = child else { continue };
        match el.name.as_str() {
            "r" => out.push_str(&run_text(el)),
            "hyperlink" => {
                // CT_Hyperlink.text: "".join(r.text for r in self.xpath("w:r")) -- direct-child
                // runs of the hyperlink, same run-text rules.
                for hchild in el.children_named("r") {
                    out.push_str(&run_text(hchild));
                }
            }
            _ => {}
        }
    }
    out
}

fn paragraph_style_id(p_el: &XmlElement) -> Option<String> {
    p_el.child_named("pPr")?.child_named("pStyle")?.attr("val").map(|s| s.to_string())
}

// ---------------------------------------------------------------------------------------------
// Table cell/grid extraction (`Table.rows` / `_Row.cells`, including `w:gridSpan` horizontal
// duplication and `w:vMerge` vertical-continuation resolution).
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum VMerge {
    Restart,
    Continue,
}

struct TcInfo {
    grid_span: usize,
    vmerge: Option<VMerge>,
    /// `_Cell.text` = `"\n".join(p.text for p in self.paragraphs)` -- DIRECT-child `<w:p>` elements
    /// of this `<w:tc>` only. Deliberately does NOT descend into a nested `<w:tbl>` child (see the
    /// module-level deviation note #2).
    own_text: String,
}

struct TrInfo {
    grid_before: usize,
    cells: Vec<TcInfo>,
}

fn parse_int_attr(el: Option<&XmlElement>, default: usize) -> usize {
    el.and_then(|e| e.attr("val")).and_then(|v| v.parse::<usize>().ok()).unwrap_or(default)
}

fn build_table_rows(tbl_el: &XmlElement) -> Vec<TrInfo> {
    tbl_el
        .children_named("tr")
        .map(|tr_el| {
            let grid_before = tr_el.child_named("trPr").map(|p| parse_int_attr(p.child_named("gridBefore"), 0)).unwrap_or(0);
            let cells = tr_el
                .children_named("tc")
                .map(|tc_el| {
                    let tc_pr = tc_el.child_named("tcPr");
                    let grid_span = tc_pr.map(|p| parse_int_attr(p.child_named("gridSpan"), 1)).unwrap_or(1);
                    let vmerge = tc_pr.and_then(|p| p.child_named("vMerge")).map(|v| match v.attr("val") {
                        Some("restart") => VMerge::Restart,
                        _ => VMerge::Continue, // element present, @val absent -> defaults to "continue"
                    });
                    // Direct-child <w:p> paragraphs only -- NOT descending into a nested <w:tbl>.
                    let own_text =
                        tc_el.children_named("p").map(paragraph_text).collect::<Vec<_>>().join("\n");
                    TcInfo { grid_span: grid_span.max(1), vmerge, own_text }
                })
                .collect();
            TrInfo { grid_before, cells }
        })
        .collect()
}

/// `CT_Row.tc_at_grid_offset`: the cell index in `row` whose grid-column starting offset (accounting
/// for `grid_before` and preceding cells' `grid_span`) exactly equals `offset`. `None` if no cell
/// starts exactly there (a malformed/inconsistent table -- python-docx raises `ValueError` in this
/// case; this returns `None` instead so callers can degrade gracefully rather than abort the whole
/// normalization over one bad table).
fn tc_at_grid_offset(row: &TrInfo, offset: usize) -> Option<usize> {
    let mut remaining = offset as i64 - row.grid_before as i64;
    for (i, tc) in row.cells.iter().enumerate() {
        if remaining < 0 {
            break;
        }
        if remaining == 0 {
            return Some(i);
        }
        remaining -= tc.grid_span as i64;
    }
    None
}

fn grid_offset(row: &TrInfo, cell_idx: usize) -> usize {
    row.grid_before + row.cells[..cell_idx].iter().map(|c| c.grid_span).sum::<usize>()
}

/// Walks a `w:vMerge="continue"` chain up to the `restart` (or un-merged) root cell, matching
/// `CT_Tc._tc_above` recursion. Returns `(row_idx, cell_idx)` of the resolved root. Falls back to
/// the starting cell itself if the chain can't be resolved (missing/malformed row above), rather
/// than erroring the whole document over one bad table.
fn resolve_root(rows: &[TrInfo], row_idx: usize, cell_idx: usize) -> (usize, usize) {
    let cell = &rows[row_idx].cells[cell_idx];
    if cell.vmerge != Some(VMerge::Continue) || row_idx == 0 {
        return (row_idx, cell_idx);
    }
    let offset = grid_offset(&rows[row_idx], cell_idx);
    match tc_at_grid_offset(&rows[row_idx - 1], offset) {
        Some(above_idx) => resolve_root(rows, row_idx - 1, above_idx),
        None => (row_idx, cell_idx), // malformed: no cell above at this offset; degrade gracefully
    }
}

/// `rows = [[collapse_whitespace(cell.text) for cell in row.cells] for row in table.rows]`, with
/// `row.cells` reproducing python-docx's `_Row.cells`: a horizontally-spanned cell's (resolved)
/// text is repeated once per spanned grid column, and a vertically-merged "continue" cell resolves
/// to its "restart" ancestor's text.
fn table_to_markdown_rows(rows: &[TrInfo]) -> Vec<Vec<String>> {
    rows.iter()
        .enumerate()
        .map(|(ri, row)| {
            let mut out = Vec::new();
            for ci in 0..row.cells.len() {
                let (root_ri, root_ci) = resolve_root(rows, ri, ci);
                let root = &rows[root_ri].cells[root_ci];
                let text = collapse_whitespace(&root.own_text);
                for _ in 0..root.grid_span.max(1) {
                    out.push(text.clone());
                }
            }
            out
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Zip / part loading
// ---------------------------------------------------------------------------------------------

fn read_zip_part(path: &Path, part_name: &str) -> Result<Option<String>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| format!("Package not found at '{}'", path.display()))?;
    let result = match archive.by_name(part_name) {
        Ok(mut entry) => {
            let mut contents = String::new();
            entry.read_to_string(&mut contents).map_err(|e| e.to_string())?;
            Ok(Some(contents))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(e) => Err(e.to_string()),
    };
    result
}

// ---------------------------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------------------------

/// `FUNCTION normalize_docx` (port-mapping-a-core-data.md), targeting current oss-launch behavior
/// (docs/port/CATCHUP_PLAN.md D1 -- confirmed unchanged from jt-dev's fork point: `normalize_docx`,
/// `heading_level_for_style`, and `markdown_table` are byte-identical in
/// `origin/integration/oss-launch:tools/sopkb/sopkb/normalize.py` and
/// `origin/jt-dev:tools/sopkb/sopkb/normalize.py`, modulo a cosmetic list-comprehension line wrap).
pub fn normalize_docx(path: &Path) -> Result<String, String> {
    let document_xml = read_zip_part(path, "word/document.xml")?
        .ok_or_else(|| "There is no item named 'word/document.xml' in the archive".to_string())?;
    let styles_xml = read_zip_part(path, "word/styles.xml")?;
    let style_table = build_style_table(styles_xml.as_deref())?;

    let document_root = parse_xml_tree(&document_xml)?;
    let body = document_root
        .child_named("body")
        .ok_or_else(|| "There is no item named 'word/document.xml' in the archive".to_string())?;

    let mut blocks: Vec<String> = Vec::new();

    // --- PASS 1: body-level paragraphs, IN DOCUMENT ORDER ---
    for p_el in body.children_named("p") {
        let text = crate::normalize::unicode_strip(&paragraph_text(p_el)).to_string();
        if text.is_empty() {
            continue;
        }
        let style_id = paragraph_style_id(p_el);
        let style_name = style_table.resolve(style_id.as_deref());
        match heading_level_for_style(&style_name) {
            Some(level) => blocks.push(format!("{} {}", "#".repeat(level as usize), text)),
            None => blocks.push(text),
        }
    }

    // --- PASS 2: top-level tables, APPENDED AFTER ALL PARAGRAPHS ---
    // (document order is DESTROYED here on purpose -- G-A17, must be preserved, not "fixed".)
    for tbl_el in body.children_named("tbl") {
        let tr_rows = build_table_rows(tbl_el);
        if tr_rows.is_empty() {
            continue;
        }
        let rows = table_to_markdown_rows(&tr_rows);
        if rows.is_empty() {
            continue;
        }
        blocks.push(markdown_table(&rows));
    }

    if blocks.is_empty() {
        return Err("DOCX text extraction produced no content".to_string());
    }
    let joined = blocks.join("\n\n");
    Ok(format!("{}\n", crate::normalize::unicode_strip(&joined)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_level_for_style_matches_g_a18_examples() {
        assert_eq!(heading_level_for_style("Heading 1"), Some(1));
        assert_eq!(heading_level_for_style("Heading  3"), Some(3)); // 2 spaces still \s+
        assert_eq!(heading_level_for_style("Heading\t2"), Some(2)); // \s+ allows tabs
        assert_eq!(heading_level_for_style("Title"), None);
        assert_eq!(heading_level_for_style("Heading 7"), None); // regex caps at 6
        assert_eq!(heading_level_for_style("heading 1"), None); // case-sensitive
        assert_eq!(heading_level_for_style("Heading 1 Char"), None); // anchored at end
        assert_eq!(heading_level_for_style("Titre 1"), None); // French, untranslated
        assert_eq!(heading_level_for_style("Überschrift 1"), None); // German, untranslated
        assert_eq!(heading_level_for_style("Heading1"), None); // no \s+ separator
    }

    #[test]
    fn babelfish_translates_only_the_documented_aliases() {
        assert_eq!(babelfish_internal2ui("heading 1"), "Heading 1");
        assert_eq!(babelfish_internal2ui("heading 9"), "Heading 9");
        assert_eq!(babelfish_internal2ui("caption"), "Caption");
        assert_eq!(babelfish_internal2ui("Titre 1"), "Titre 1"); // unrecognized -> passthrough
        assert_eq!(babelfish_internal2ui("Normal"), "Normal");
    }

    #[test]
    fn collapse_whitespace_matches_python_str_split_join() {
        assert_eq!(collapse_whitespace("  a  b\nc\t d "), "a b c d");
        assert_eq!(collapse_whitespace(""), "");
        assert_eq!(collapse_whitespace("   "), "");
    }

    #[test]
    fn markdown_table_matches_pseudocode_shape() {
        let rows = vec![
            vec!["A".to_string(), "B".to_string()],
            vec!["1".to_string()], // short row gets padded
        ];
        assert_eq!(markdown_table(&rows), "| A | B |\n| --- | --- |\n| 1 |  |");
    }

    #[test]
    fn run_text_handles_tab_and_line_break() {
        let el = parse_xml_tree(
            r#"<w:r xmlns:w="w"><w:t>a</w:t><w:tab/><w:t>b</w:t><w:br/><w:t>c</w:t></w:r>"#,
        )
        .unwrap();
        assert_eq!(run_text(&el), "a\tb\nc");
    }

    #[test]
    fn run_text_page_break_contributes_nothing() {
        let el = parse_xml_tree(
            r#"<w:r xmlns:w="w"><w:t>a</w:t><w:br w:type="page"/><w:t>b</w:t></w:r>"#,
        )
        .unwrap();
        assert_eq!(run_text(&el), "ab");
    }
}
