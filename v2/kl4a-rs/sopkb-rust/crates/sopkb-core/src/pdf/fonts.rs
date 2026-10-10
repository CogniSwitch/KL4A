//! Font handling: code -> width and code -> Unicode, mirroring pdfminer.six's
//! `pdffont.py` closely enough that glyph advances (and therefore every
//! downstream word/line break) match.
//!
//! Covered: simple single-byte fonts (Type1, TrueType, Type3) with `/Widths`,
//! base-14 fonts with no `/Widths` (via the generated AFM tables), the three
//! base encodings plus `/Differences`, and `/ToUnicode` CMaps.
//!
//! Composite (Type0/CID) fonts are covered for the shape that dominates real
//! documents: `Identity-H`/`Identity-V` 2-byte codes, `/W` widths (both the
//! `c [w1 w2 ...]` and `c_first c_last w` forms), `/DW`, the descendant font's
//! `/FontDescriptor` descent, and a `/ToUnicode` CMap. This is verified
//! character-for-character against pdfplumber by the `type0-identity-h` case in
//! `fixtures/harness/diff_pdf_extraction.py`.
//!
//! NOT covered: a Type0 font using a non-identity predefined or embedded CMap.
//! Such a font still gets the 2-byte identity decode, which would mis-split
//! codes for a CMap with mixed byte lengths. Implementing it means a real CMap
//! interpreter (codespace ranges + cidrange mapping), deferred deliberately.

use std::collections::HashMap;

use lopdf::{Dictionary, Document, Object};

use super::tables;

/// The subset of pdfminer's `PDFFont` this port reads.
#[derive(Debug, Clone)]
pub struct Font {
    /// pdfminer's `font.fontname`, which pdfplumber surfaces as `char["fontname"]`.
    pub fontname: String,
    /// Glyph-space widths (1/1000 em) keyed by character code.
    widths: HashMap<u32, f64>,
    /// `/MissingWidth`, or the base-14 fallback.
    default_width: f64,
    /// `/Descent`, forced negative the way pdfminer does.
    descent: f64,
    /// `/ToUnicode`-derived text, keyed by code.
    to_unicode: HashMap<u32, String>,
    /// Code -> glyph name, for simple fonts (base encoding + `/Differences`).
    code_to_glyph: Vec<Option<String>>,
    /// Base-14 AFM widths, keyed by the Unicode character (see `tables`).
    std14_widths: Option<&'static [(&'static str, i32)]>,
    /// Composite fonts consume two bytes per code and skip word-spacing.
    pub is_multibyte: bool,
}

impl Font {
    /// pdfminer `char_width(cid)`: try the code, then the code's Unicode text,
    /// then the default. Returns text-space units (already scaled by 1/1000).
    pub fn char_width(&self, code: u32) -> f64 {
        if let Some(w) = self.widths.get(&code) {
            return w * 0.001;
        }
        if let Some(std) = self.std14_widths {
            if let Some(text) = self.to_unichr(code) {
                if let Some(w) = tables::standard_width(std, &text) {
                    return f64::from(w) * 0.001;
                }
            }
        }
        self.default_width * 0.001
    }

    /// pdfminer `get_descent()`, in text-space units.
    pub fn descent(&self) -> f64 {
        self.descent * 0.001
    }

    /// pdfminer `to_unichr(cid)`. `None` where pdfminer would raise
    /// `PDFUnicodeNotDefined` -- the caller decides what to emit.
    pub fn to_unichr(&self, code: u32) -> Option<String> {
        if let Some(t) = self.to_unicode.get(&code) {
            return Some(t.clone());
        }
        let glyph = self.code_to_glyph.get(code as usize).and_then(|g| g.clone());
        if let Some(name) = glyph {
            if let Some(c) = tables::glyph_to_unicode(&name) {
                return Some(c.to_string());
            }
        }
        None
    }

    /// Split a PDF string into character codes.
    pub fn decode(&self, bytes: &[u8]) -> Vec<u32> {
        if self.is_multibyte {
            bytes.chunks(2).map(|c| if c.len() == 2 { (u32::from(c[0]) << 8) | u32::from(c[1]) } else { u32::from(c[0]) }).collect()
        } else {
            bytes.iter().map(|&b| u32::from(b)).collect()
        }
    }

    /// Build a `Font` from a PDF font dictionary.
    pub fn from_dict(doc: &Document, dict: &Dictionary) -> Font {
        let subtype = name_of(doc, dict.get(b"Subtype").ok()).unwrap_or_default();
        let basefont = name_of(doc, dict.get(b"BaseFont").ok()).unwrap_or_else(|| "unknown".to_string());

        if subtype == "Type0" {
            return Self::composite(doc, dict, basefont);
        }

        let descriptor = dict
            .get(b"FontDescriptor")
            .ok()
            .and_then(|o| resolve_dict(doc, o));

        let std14 = tables::standard_14(&basefont);
        // pdfminer prefers the base-14 metrics over the embedded /Widths when
        // the BaseFont name is a known standard font -- the `try` around
        // `FontMetricsDB.get_metrics` comes first, and only a KeyError falls
        // through to /Widths.
        let (std14_widths, mut descent, default_width) = match std14 {
            Some((w, _ascent, d, _bbox)) => (Some(w), d, 0.0),
            None => {
                let d = descriptor
                    .as_ref()
                    .and_then(|fd| number(doc, fd.get(b"Descent").ok()))
                    .unwrap_or(0.0);
                let mw = descriptor
                    .as_ref()
                    .and_then(|fd| number(doc, fd.get(b"MissingWidth").ok()))
                    .unwrap_or(0.0);
                (None, d, mw)
            }
        };
        // "PDF RM 9.8.1 specifies /Descent should always be a negative number.
        // PScript5.dll seems to produce Descent with a positive number" --
        // pdfminer forces it negative, and so must we or every glyph's vertical
        // box (and thus `top`) shifts.
        if descent > 0.0 {
            descent = -descent;
        }

        let mut widths = HashMap::new();
        if std14_widths.is_none() {
            let first_char = number(doc, dict.get(b"FirstChar").ok()).unwrap_or(0.0) as i64;
            if let Some(Object::Array(arr)) = dict.get(b"Widths").ok().and_then(|o| resolve(doc, o)) {
                for (i, w) in arr.iter().enumerate() {
                    if let Some(v) = obj_number(doc, w) {
                        widths.insert((first_char + i as i64).max(0) as u32, v);
                    }
                }
            }
        }

        let code_to_glyph = Self::build_encoding(doc, dict, &subtype, &basefont);
        let to_unicode = Self::build_to_unicode(doc, dict);

        Font {
            fontname: basefont,
            widths,
            default_width,
            descent,
            to_unicode,
            code_to_glyph,
            std14_widths,
            is_multibyte: false,
        }
    }

    /// Minimal composite-font handling. Deliberately partial -- see the module
    /// docs. `/W` is parsed (both `c [w1 w2 ...]` and `c_first c_last w` forms)
    /// because without it every CID glyph would get the same advance.
    fn composite(doc: &Document, dict: &Dictionary, basefont: String) -> Font {
        let mut widths = HashMap::new();
        let mut default_width = 1000.0;
        let mut descent = 0.0;

        if let Some(Object::Array(descendants)) = dict.get(b"DescendantFonts").ok().and_then(|o| resolve(doc, o)) {
            if let Some(df) = descendants.first().and_then(|o| resolve_dict(doc, o)) {
                if let Some(dw) = number(doc, df.get(b"DW").ok()) {
                    default_width = dw;
                }
                if let Some(fd) = df.get(b"FontDescriptor").ok().and_then(|o| resolve_dict(doc, o)) {
                    descent = number(doc, fd.get(b"Descent").ok()).unwrap_or(0.0);
                }
                if let Some(Object::Array(w)) = df.get(b"W").ok().and_then(|o| resolve(doc, o)) {
                    parse_cid_widths(doc, &w, &mut widths);
                }
            }
        }
        if descent > 0.0 {
            descent = -descent;
        }

        Font {
            fontname: basefont,
            widths,
            default_width,
            descent,
            to_unicode: Self::build_to_unicode(doc, dict),
            code_to_glyph: vec![None; 256],
            std14_widths: None,
            is_multibyte: true,
        }
    }

    /// Base encoding + `/Differences`, as a 256-entry code -> glyph-name table.
    fn build_encoding(doc: &Document, dict: &Dictionary, subtype: &str, basefont: &str) -> Vec<Option<String>> {
        // Symbol and ZapfDingbats carry their own built-in encodings; pdfminer
        // leaves them to the font's own table rather than applying a Latin one.
        // Approximated here by starting from StandardEncoding, which is what a
        // non-symbolic default would give.
        let base_default: &[Option<&str>; 256] = if subtype == "TrueType" {
            &tables::WIN_ANSI_ENCODING
        } else {
            &tables::STANDARD_ENCODING
        };

        let mut base: Vec<Option<String>> = base_default.iter().map(|o| o.map(str::to_string)).collect();
        let _ = basefont;

        let enc_obj = dict.get(b"Encoding").ok().and_then(|o| resolve(doc, o));
        match enc_obj {
            Some(Object::Name(n)) => {
                if let Some(t) = named_encoding(&String::from_utf8_lossy(&n)) {
                    base = t.iter().map(|o| o.map(str::to_string)).collect();
                }
            }
            Some(Object::Dictionary(d)) => {
                if let Some(name) = name_of(doc, d.get(b"BaseEncoding").ok()) {
                    if let Some(t) = named_encoding(&name) {
                        base = t.iter().map(|o| o.map(str::to_string)).collect();
                    }
                }
                if let Some(Object::Array(diffs)) = d.get(b"Differences").ok().and_then(|o| resolve(doc, o)) {
                    let mut code: i64 = 0;
                    for item in &diffs {
                        match resolve(doc, item) {
                            Some(Object::Integer(i)) => code = i,
                            Some(Object::Real(r)) => code = r as i64,
                            Some(Object::Name(n)) => {
                                if (0..256).contains(&code) {
                                    base[code as usize] = Some(String::from_utf8_lossy(&n).to_string());
                                }
                                code += 1;
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        base
    }

    /// Parse a `/ToUnicode` CMap's `beginbfchar`/`beginbfrange` sections.
    fn build_to_unicode(doc: &Document, dict: &Dictionary) -> HashMap<u32, String> {
        let mut out = HashMap::new();
        let Some(obj) = dict.get(b"ToUnicode").ok() else { return out };
        let Some(resolved) = resolve(doc, obj) else { return out };
        let Object::Stream(stream) = resolved else { return out };
        let Ok(data) = stream.decompressed_content().or_else(|_| Ok::<_, lopdf::Error>(stream.content.clone())) else {
            return out;
        };
        parse_tounicode_cmap(&String::from_utf8_lossy(&data), &mut out);
        out
    }
}

fn named_encoding(name: &str) -> Option<&'static [Option<&'static str>; 256]> {
    match name {
        "WinAnsiEncoding" => Some(&tables::WIN_ANSI_ENCODING),
        "MacRomanEncoding" => Some(&tables::MAC_ROMAN_ENCODING),
        "StandardEncoding" | "PDFDocEncoding" => Some(&tables::STANDARD_ENCODING),
        _ => None,
    }
}

fn parse_cid_widths(doc: &Document, w: &[Object], widths: &mut HashMap<u32, f64>) {
    let mut i = 0;
    while i < w.len() {
        let Some(first) = obj_number(doc, &w[i]) else { break };
        if i + 1 >= w.len() {
            break;
        }
        match resolve(doc, &w[i + 1]) {
            Some(Object::Array(list)) => {
                for (k, item) in list.iter().enumerate() {
                    if let Some(v) = obj_number(doc, item) {
                        widths.insert(first as u32 + k as u32, v);
                    }
                }
                i += 2;
            }
            _ => {
                if i + 2 >= w.len() {
                    break;
                }
                let last = obj_number(doc, &w[i + 1]).unwrap_or(first);
                let val = obj_number(doc, &w[i + 2]).unwrap_or(0.0);
                let (lo, hi) = (first as i64, last as i64);
                if hi >= lo && hi - lo < 65_536 {
                    for c in lo..=hi {
                        widths.insert(c as u32, val);
                    }
                }
                i += 3;
            }
        }
    }
}

/// Parse the `beginbfchar`/`beginbfrange` blocks of a `/ToUnicode` CMap.
///
/// A deliberately small, tolerant parser rather than a full PostScript CMap
/// interpreter: these two operators carry essentially all of the code->text
/// mapping in practice, and an unparseable CMap degrades to the encoding-based
/// path rather than failing the page.
fn parse_tounicode_cmap(text: &str, out: &mut HashMap<u32, String>) {
    let toks: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < toks.len() {
        match toks[i] {
            "beginbfchar" => {
                i += 1;
                while i + 1 < toks.len() && toks[i] != "endbfchar" {
                    if let (Some(src), Some(dst)) = (hex_token(toks[i]), hex_token(toks[i + 1])) {
                        if let Some(code) = hex_to_u32(&src) {
                            out.insert(code, utf16be_to_string(&dst));
                        }
                    }
                    i += 2;
                }
            }
            "beginbfrange" => {
                i += 1;
                while i < toks.len() && toks[i] != "endbfrange" {
                    if i + 2 >= toks.len() {
                        break;
                    }
                    // The destination may be a single value or a `[...]` list;
                    // the list form is skipped rather than mis-parsed.
                    if toks[i + 2].starts_with('[') {
                        while i < toks.len() && !toks[i].ends_with(']') {
                            i += 1;
                        }
                        i += 1;
                        continue;
                    }
                    let (lo, hi, dst) = (hex_token(toks[i]), hex_token(toks[i + 1]), hex_token(toks[i + 2]));
                    if let (Some(lo), Some(hi), Some(dst)) = (lo, hi, dst) {
                        if let (Some(lo), Some(hi)) = (hex_to_u32(&lo), hex_to_u32(&hi)) {
                            let base = utf16be_to_string(&dst);
                            let base_cp = base.chars().next().map(u32::from);
                            if hi >= lo && hi - lo < 65_536 {
                                for (n, code) in (lo..=hi).enumerate() {
                                    match base_cp.and_then(|b| char::from_u32(b + n as u32)) {
                                        Some(c) => {
                                            out.insert(code, c.to_string());
                                        }
                                        None => {
                                            out.insert(code, base.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    i += 3;
                }
            }
            _ => i += 1,
        }
    }
}

/// `<0041>` -> the bytes `41`. Returns `None` for a non-hex token.
fn hex_token(tok: &str) -> Option<Vec<u8>> {
    let inner = tok.strip_prefix('<')?.strip_suffix('>')?;
    if inner.is_empty() || inner.len() % 2 != 0 || !inner.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    (0..inner.len()).step_by(2).map(|i| u8::from_str_radix(&inner[i..i + 2], 16).ok()).collect()
}

fn hex_to_u32(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() || bytes.len() > 4 {
        return None;
    }
    Some(bytes.iter().fold(0u32, |acc, &b| (acc << 8) | u32::from(b)))
}

/// A `/ToUnicode` destination is UTF-16BE, and may be a surrogate pair or even
/// several codepoints (a ligature mapping back to "ffi").
fn utf16be_to_string(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.chunks(2).map(|c| if c.len() == 2 { (u16::from(c[0]) << 8) | u16::from(c[1]) } else { u16::from(c[0]) }).collect();
    String::from_utf16_lossy(&units)
}

// --- small lopdf helpers -------------------------------------------------

pub(crate) fn resolve<'a>(doc: &'a Document, obj: &'a Object) -> Option<Object> {
    doc.dereference(obj).ok().map(|(_, o)| o.clone())
}

pub(crate) fn resolve_dict(doc: &Document, obj: &Object) -> Option<Dictionary> {
    match resolve(doc, obj)? {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(s.dict.clone()),
        _ => None,
    }
}

fn name_of(doc: &Document, obj: Option<&Object>) -> Option<String> {
    match resolve(doc, obj?)? {
        Object::Name(n) => Some(String::from_utf8_lossy(&n).to_string()),
        _ => None,
    }
}

fn number(doc: &Document, obj: Option<&Object>) -> Option<f64> {
    obj_number(doc, obj?)
}

fn obj_number(doc: &Document, obj: &Object) -> Option<f64> {
    match resolve(doc, obj)? {
        Object::Integer(i) => Some(i as f64),
        Object::Real(r) => Some(f64::from(r)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_token_parses_and_rejects() {
        assert_eq!(hex_token("<0041>"), Some(vec![0x00, 0x41]));
        assert_eq!(hex_token("<41>"), Some(vec![0x41]));
        assert_eq!(hex_token("0041"), None);
        assert_eq!(hex_token("<041>"), None, "odd digit count is not a valid hex string");
        assert_eq!(hex_token("<zz>"), None);
    }

    #[test]
    fn utf16be_decodes_bmp_and_surrogates() {
        assert_eq!(utf16be_to_string(&[0x00, 0x41]), "A");
        // U+1F600 as a surrogate pair.
        assert_eq!(utf16be_to_string(&[0xD8, 0x3D, 0xDE, 0x00]), "\u{1F600}");
        // Multi-codepoint destination (a ligature mapped back to its letters).
        assert_eq!(utf16be_to_string(&[0x00, 0x66, 0x00, 0x69]), "fi");
    }

    #[test]
    fn tounicode_bfchar_and_bfrange() {
        let cmap = "
        2 beginbfchar
        <01> <0041>
        <02> <0042>
        endbfchar
        1 beginbfrange
        <10> <12> <0061>
        endbfrange
        ";
        let mut out = HashMap::new();
        parse_tounicode_cmap(cmap, &mut out);
        assert_eq!(out.get(&0x01).map(String::as_str), Some("A"));
        assert_eq!(out.get(&0x02).map(String::as_str), Some("B"));
        assert_eq!(out.get(&0x10).map(String::as_str), Some("a"));
        assert_eq!(out.get(&0x11).map(String::as_str), Some("b"));
        assert_eq!(out.get(&0x12).map(String::as_str), Some("c"));
        assert_eq!(out.get(&0x13), None, "range is inclusive of hi and stops there");
    }

    #[test]
    fn standard_14_widths_are_keyed_by_unicode_char() {
        // Guards the correction that `FONT_METRICS` is keyed by the character,
        // not the glyph name -- getting this backwards silently falls back to
        // default_width for every glyph and shifts the whole page.
        let (w, ..) = tables::standard_14("Helvetica").expect("Helvetica is a base-14 font");
        assert_eq!(tables::standard_width(w, " "), Some(278));
        assert_eq!(tables::standard_width(w, "H"), Some(722));
        assert_eq!(tables::standard_width(w, "space"), None);
    }

    #[test]
    fn standard_14_resolves_aliases_and_subset_prefixes() {
        assert!(tables::standard_14("ArialMT").is_some());
        assert!(tables::standard_14("ABCDEF+Helvetica-Bold").is_some());
        assert!(tables::standard_14("NoSuchFont").is_none());
    }

    #[test]
    fn win_ansi_encoding_maps_the_usual_suspects() {
        assert_eq!(tables::WIN_ANSI_ENCODING[65], Some("A"));
        assert_eq!(tables::WIN_ANSI_ENCODING[32], Some("space"));
        assert_eq!(tables::glyph_to_unicode("space"), Some(' '));
        assert_eq!(tables::glyph_to_unicode("eacute"), Some('é'));
        assert_eq!(tables::glyph_to_unicode("uni0041"), Some('A'));
    }
}
