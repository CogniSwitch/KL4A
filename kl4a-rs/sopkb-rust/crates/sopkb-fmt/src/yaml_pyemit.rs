//! Minimal PyYAML-compatible value type, parser, and emitter, scoped to exactly the
//! shape `manifest.yaml` uses: a top-level block mapping whose values are plain/quoted
//! scalar strings or block sequences of block mappings-of-scalars (docs/port/
//! port-mapping-a-core-data.md §3.3 `save_manifest`/`load_manifest`). This is NOT a
//! general-purpose YAML library -- it exists to reproduce PyYAML's
//! `yaml.safe_dump(sort_keys=False, allow_unicode=False)` output byte-for-byte for
//! this one shape, which no off-the-shelf Rust YAML crate does (they all use their
//! own emitter heuristics).

use crate::ordered_map::OrderedMap;
use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Debug, Clone, PartialEq)]
pub enum YamlValue {
    /// A string value. Quoting is CHOSEN at emit time (single/double/plain) based on
    /// whether the text would otherwise resolve to a different YAML type -- this is
    /// what `yaml.safe_dump` does for genuine Python `str` values.
    Scalar(String),
    /// An already-final plain-scalar token for a genuinely non-string Python value
    /// (`None`/`bool`/`int`/`float`) -- e.g. `"null"`, `"true"`, `"0.82"`, `"42"`.
    /// Emitted verbatim, unquoted, with folding but WITHOUT the Scalar quoting
    /// heuristic (PyYAML never quotes a real int/float/bool/None just because its
    /// text form resembles one -- that heuristic exists only to protect a genuine
    /// `str` from being misread as another type). `sopkb-mining`'s OKF frontmatter
    /// writer builds these via `ordered_json_to_yaml`, pre-formatting the token itself
    /// (`"true"`/`"false"`/`"null"`/`py_float(f)`/decimal digits) before wrapping it.
    Raw(String),
    Sequence(Vec<YamlValue>),
    Mapping(OrderedMap<YamlValue>),
}

impl YamlValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            YamlValue::Scalar(s) | YamlValue::Raw(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_mapping(&self) -> Option<&OrderedMap<YamlValue>> {
        match self {
            YamlValue::Mapping(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_sequence(&self) -> Option<&[YamlValue]> {
        match self {
            YamlValue::Sequence(s) => Some(s),
            _ => None,
        }
    }
}

// --- emitter --------------------------------------------------------------------------

/// Matches `yaml.safe_dump(value, sort_keys=False, allow_unicode=False)`: block style,
/// sequences NOT indented under their key, plain scalars preferred, single-quoted for
/// anything that would otherwise resolve to a non-string type or contains YAML-unsafe
/// syntax, double-quoted (with `\xNN`/`\uNNNN`/`\UNNNNNNNN` escapes) for anything
/// containing non-ASCII or control characters. Ends with exactly one trailing newline.
pub fn emit(value: &YamlValue) -> String {
    let YamlValue::Mapping(map) = value else {
        panic!("top-level emit() requires a Mapping");
    };
    let mut out = String::new();
    emit_mapping_block(&mut out, map, 0);
    out
}

fn indent_str(n: usize) -> String {
    " ".repeat(n)
}

fn emit_mapping_block(out: &mut String, map: &OrderedMap<YamlValue>, indent: usize) {
    for (key, value) in map.iter() {
        out.push_str(&indent_str(indent));
        let key_repr = scalar_repr(key);
        out.push_str(&key_repr);
        out.push(':');
        let column_after_colon = indent + key_repr.chars().count() + 1;
        emit_value_after_colon(out, value, indent, column_after_colon);
    }
}

fn emit_value_after_colon(out: &mut String, value: &YamlValue, indent: usize, column_after_colon: usize) {
    match value {
        YamlValue::Scalar(s) => {
            out.push(' ');
            write_scalar_value(out, s, column_after_colon + 1, indent + 2);
            out.push('\n');
        }
        YamlValue::Raw(s) => {
            out.push(' ');
            out.push_str(&fold_plain(s, column_after_colon + 1, indent + 2));
            out.push('\n');
        }
        YamlValue::Sequence(items) => {
            if items.is_empty() {
                out.push_str(" []\n");
            } else {
                out.push('\n');
                for item in items {
                    emit_sequence_item(out, item, indent);
                }
            }
        }
        YamlValue::Mapping(nested) => {
            if nested.is_empty() {
                out.push_str(" {}\n");
            } else {
                out.push('\n');
                emit_mapping_block(out, nested, indent + 2);
            }
        }
    }
}

fn emit_sequence_item(out: &mut String, item: &YamlValue, indent: usize) {
    out.push_str(&indent_str(indent));
    out.push_str("- ");
    match item {
        YamlValue::Mapping(map) => {
            let mut first = true;
            for (key, value) in map.iter() {
                if !first {
                    out.push_str(&indent_str(indent + 2));
                }
                let key_repr = scalar_repr(key);
                out.push_str(&key_repr);
                out.push(':');
                let item_indent = indent + 2;
                let column_after_colon = item_indent + key_repr.chars().count() + 1;
                emit_value_after_colon(out, value, item_indent, column_after_colon);
                first = false;
            }
        }
        YamlValue::Scalar(s) => {
            write_scalar_value(out, s, indent + 2, indent + 2);
            out.push('\n');
        }
        YamlValue::Raw(s) => {
            out.push_str(&fold_plain(s, indent + 2, indent + 2));
            out.push('\n');
        }
        YamlValue::Sequence(_) => {
            // Nested sequence-of-sequences directly under "- " never occurs in our
            // domain (manifest.yaml or OKF frontmatter); represented minimally rather
            // than silently dropping data.
            out.push_str("[]\n");
        }
    }
}

static RE_BOOL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:yes|Yes|YES|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF)$").unwrap()
});
static RE_NULL: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(?:~|null|Null|NULL)$").unwrap());
static RE_INT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:[-+]?0b[0-1_]+|[-+]?0[0-7_]+|[-+]?(?:0|[1-9][0-9_]*)|[-+]?0x[0-9a-fA-F_]+)$").unwrap()
});
static RE_FLOAT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?:[-+]?(?:[0-9][0-9_]*)\.[0-9_]*(?:[eE][-+]?[0-9]+)?|\.[0-9][0-9_]*(?:[eE][-+]?[0-9]+)?|[-+]?\.(?:inf|Inf|INF)|\.(?:nan|NaN|NAN))$",
    )
    .unwrap()
});
static RE_TIMESTAMP: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$|^[0-9][0-9][0-9][0-9]-[0-9][0-9]?-[0-9][0-9]?([Tt]|[ \t]+)[0-9][0-9]?:[0-9][0-9]:[0-9][0-9](\.[0-9]*)?(([ \t]*)Z|[-+][0-9][0-9]?(:[0-9][0-9])?)?$",
    )
    .unwrap()
});

pub fn scalar_repr(s: &str) -> String {
    if s.chars().any(|c| (c as u32) > 0x7E || (c as u32) < 0x20) {
        return double_quoted(s);
    }
    if s.is_empty() {
        return "''".to_string();
    }
    if looks_like_non_string(s) || needs_quoting_for_syntax(s) {
        return single_quoted(s);
    }
    s.to_string()
}

fn looks_like_non_string(s: &str) -> bool {
    RE_BOOL.is_match(s)
        || RE_NULL.is_match(s)
        || RE_INT.is_match(s)
        || RE_FLOAT.is_match(s)
        || RE_TIMESTAMP.is_match(s)
}

fn needs_quoting_for_syntax(s: &str) -> bool {
    let first = s.chars().next().unwrap();
    let last = s.chars().last().unwrap();
    if first.is_whitespace() || last.is_whitespace() {
        return true;
    }
    if "-?:,[]{}#&*!|>'\"%@`".contains(first) {
        return true;
    }
    if s.contains(": ") || s.ends_with(':') || s.contains(" #") {
        return true;
    }
    false
}

fn single_quoted(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn escape_double_quoted_char(ch: char) -> String {
    let cp = ch as u32;
    match ch {
        '"' => "\\\"".to_string(),
        '\\' => "\\\\".to_string(),
        '\n' => "\\n".to_string(),
        '\t' => "\\t".to_string(),
        '\r' => "\\r".to_string(),
        _ if cp < 0x20 || cp == 0x7F => format!("\\x{cp:02X}"),
        _ if cp <= 0xFF => format!("\\x{cp:02X}"),
        _ if cp <= 0xFFFF => format!("\\u{cp:04X}"),
        _ => format!("\\U{cp:08X}"),
    }
}

fn is_safe_double_quoted_char(ch: char) -> bool {
    let cp = ch as u32;
    (0x20..=0x7E).contains(&cp) && ch != '"' && ch != '\\'
}

fn double_quoted(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        if is_safe_double_quoted_char(ch) {
            out.push(ch);
        } else {
            out.push_str(&escape_double_quoted_char(ch));
        }
    }
    out.push('"');
    out
}

// --- PyYAML-style 80-column scalar folding (yaml.safe_dump default best_width=80) ------
//
// Ported line-for-line from CPython PyYAML's `Emitter.write_plain` /
// `write_single_quoted` / `write_double_quoted` (site-packages/yaml/emitter.py).
// `manifest.yaml` fields are always short enough that these degrade to a no-op
// (verified by the pre-existing `scalar_repr` unit tests, which are unaffected), but
// OKF frontmatter descriptions/objects routinely exceed 80 columns -- see
// docs/port/port-mapping-c-review-export.md §0.3/G13. `column` is the 0-based column
// at which the scalar's first emitted character (including any opening quote) lands;
// `indent` is the continuation indent for wrapped lines (the enclosing block's own
// indent + 2, i.e. "indented by 2 relative to the key").

const BEST_WIDTH: usize = 80;

fn is_break_char(c: char) -> bool {
    matches!(c, '\n' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// Mirrors `Emitter.write_plain` (no leading separator space -- the caller already
/// wrote the space after "key:"; `column` is the column right after that space).
fn fold_plain(text: &str, column: usize, indent: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::new();
    let mut column = column;
    let mut spaces = false;
    let mut breaks = false;
    let mut start = 0usize;
    let mut end = 0usize;
    while end <= n {
        let ch = if end < n { Some(chars[end]) } else { None };
        if spaces {
            if ch != Some(' ') {
                if start + 1 == end && column > BEST_WIDTH {
                    out.push('\n');
                    out.push_str(&indent_str(indent));
                    column = indent;
                } else {
                    let seg: String = chars[start..end].iter().collect();
                    column += end - start;
                    out.push_str(&seg);
                }
                start = end;
            }
        } else if breaks {
            if ch.is_none_or(|c| !is_break_char(c)) {
                if chars[start] == '\n' {
                    out.push('\n');
                }
                for &br in &chars[start..end] {
                    out.push(if br == '\n' { '\n' } else { br });
                }
                out.push_str(&indent_str(indent));
                column = indent;
                start = end;
            }
        } else if ch.is_none() || ch == Some(' ') || ch.is_some_and(is_break_char) {
            let seg: String = chars[start..end].iter().collect();
            column += end - start;
            out.push_str(&seg);
            start = end;
        }
        if let Some(c) = ch {
            spaces = c == ' ';
            breaks = is_break_char(c);
        }
        end += 1;
    }
    out
}

/// Mirrors `Emitter.write_single_quoted` (includes the surrounding `'...'`).
fn fold_single_quoted(text: &str, column: usize, indent: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::from("'");
    let mut column = column + 1;
    let mut spaces = false;
    let mut breaks = false;
    let mut start = 0usize;
    let mut end = 0usize;
    while end <= n {
        let ch = if end < n { Some(chars[end]) } else { None };
        if spaces {
            if ch.is_none() || ch != Some(' ') {
                if start + 1 == end && column > BEST_WIDTH && start != 0 && end != n {
                    out.push('\n');
                    out.push_str(&indent_str(indent));
                    column = indent;
                } else {
                    let seg: String = chars[start..end].iter().collect();
                    column += end - start;
                    out.push_str(&seg);
                }
                start = end;
            }
        } else if breaks {
            if ch.is_none_or(|c| !is_break_char(c)) {
                if chars[start] == '\n' {
                    out.push('\n');
                }
                for &br in &chars[start..end] {
                    out.push(if br == '\n' { '\n' } else { br });
                }
                out.push_str(&indent_str(indent));
                column = indent;
                start = end;
            }
        } else if (ch.is_none() || ch == Some(' ') || ch.is_some_and(is_break_char) || ch == Some('\''))
            && start < end {
                let seg: String = chars[start..end].iter().collect();
                column += end - start;
                out.push_str(&seg);
                start = end;
            }
        if ch == Some('\'') {
            out.push_str("''");
            column += 2;
            start = end + 1;
        }
        if let Some(c) = ch {
            spaces = c == ' ';
            breaks = is_break_char(c);
        }
        end += 1;
    }
    out.push('\'');
    out
}

/// Mirrors `Emitter.write_double_quoted` (includes the surrounding `"..."`).
/// `allow_unicode=False` is baked in (every non-ASCII-printable char is escaped).
fn fold_double_quoted(text: &str, column: usize, indent: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::from("\"");
    let mut column = column + 1;
    let mut start = 0usize;
    let mut end = 0usize;
    while end <= n {
        let ch = if end < n { Some(chars[end]) } else { None };
        let needs_escape = match ch {
            None => true,
            Some(c) => !is_safe_double_quoted_char(c),
        };
        if needs_escape {
            if start < end {
                let seg: String = chars[start..end].iter().collect();
                column += end - start;
                out.push_str(&seg);
                start = end;
            }
            if let Some(c) = ch {
                let esc = escape_double_quoted_char(c);
                column += esc.chars().count();
                out.push_str(&esc);
                start = end + 1;
            }
        }
        // `start` can exceed `end` here (an escape just set `start = end + 1`),
        // mirroring Python's `text[start:end]` (empty when start>end) and
        // `column + (end - start)` (may subtract a negative span) -- use signed
        // arithmetic for the width comparison and clamp the slice to non-negative.
        if end > 0
            && end + 1 < n
            && (ch == Some(' ') || start >= end)
            && column as isize + (end as isize - start as isize) > BEST_WIDTH as isize
        {
            let mut data: String = if start < end { chars[start..end].iter().collect() } else { String::new() };
            data.push('\\');
            if start < end {
                start = end;
            }
            out.push_str(&data);
            out.push('\n');
            out.push_str(&indent_str(indent));
            // Matches `write_indent()`: unconditionally breaks to a new line then
            // pads to `indent` when mid-scalar, so the pre-break column value above
            // is never observed again -- no separate increment needed here.
            column = indent;
            if chars[start] == ' ' {
                out.push('\\');
                column += 1;
            }
        }
        end += 1;
    }
    out.push('"');
    out
}

/// Chooses style exactly as `scalar_repr` does, then renders with 80-column folding.
/// Used for mapping/sequence VALUES (never keys -- OKF/manifest keys are always short
/// identifiers, so folding them is unobserved and `scalar_repr` is left as the
/// unfolded renderer for keys to avoid touching Phase 1-4 golden-tested behavior).
fn write_scalar_value(out: &mut String, s: &str, column: usize, indent: usize) {
    if s.chars().any(|c| (c as u32) > 0x7E || (c as u32) < 0x20) {
        out.push_str(&fold_double_quoted(s, column, indent));
        return;
    }
    if s.is_empty() {
        out.push_str("''");
        return;
    }
    if looks_like_non_string(s) || needs_quoting_for_syntax(s) {
        out.push_str(&fold_single_quoted(s, column, indent));
        return;
    }
    out.push_str(&fold_plain(s, column, indent));
}

// --- parser ---------------------------------------------------------------------------

struct Parser<'a> {
    lines: Vec<&'a str>,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        Self { lines, pos: 0 }
    }

    fn peek(&self) -> Option<&'a str> {
        self.lines.get(self.pos).copied()
    }

    fn indent_of(line: &str) -> usize {
        line.len() - line.trim_start_matches(' ').len()
    }

    fn parse_mapping(&mut self, indent: usize) -> Result<OrderedMap<YamlValue>, String> {
        let mut map = OrderedMap::new();
        while let Some(line) = self.peek() {
            let line_indent = Self::indent_of(line);
            if line_indent != indent {
                break;
            }
            let trimmed = &line[indent..];
            if trimmed.starts_with("- ") || trimmed == "-" {
                break; // sequence content; caller handles this
            }
            let colon = find_key_colon(trimmed)
                .ok_or_else(|| format!("expected 'key: value', got: {line:?}"))?;
            let key = unquote_scalar(trimmed[..colon].trim());
            let rest = trimmed[colon + 1..].trim_start();
            self.pos += 1;
            if rest.is_empty() {
                if let Some(next) = self.peek() {
                    let next_indent = Self::indent_of(next);
                    let next_trimmed = &next[next_indent.min(next.len())..];
                    // A block sequence's "- " markers may sit at the SAME indent as
                    // their key (PyYAML's own dumper convention, e.g. `sources:\n- id:
                    // ...`) OR indented further (conventional hand-written YAML, e.g.
                    // `sources:\n  - id: ...`) -- both are valid YAML and PyYAML's
                    // *loader* (unlike its dumper) accepts either, so this parser must
                    // too rather than assuming its own emitter's specific convention.
                    if next_indent >= indent && (next_trimmed.starts_with("- ") || next_trimmed == "-") {
                        let seq = self.parse_sequence(next_indent)?;
                        map.insert(key, YamlValue::Sequence(seq));
                        continue;
                    } else if next_indent > indent {
                        let nested = self.parse_mapping(next_indent)?;
                        map.insert(key, YamlValue::Mapping(nested));
                        continue;
                    }
                }
                map.insert(key, YamlValue::Scalar(String::new()));
            } else if rest == "[]" {
                map.insert(key, YamlValue::Sequence(Vec::new()));
            } else if rest == "{}" {
                map.insert(key, YamlValue::Mapping(OrderedMap::new()));
            } else {
                map.insert(key, YamlValue::Scalar(unquote_scalar(rest)));
            }
        }
        Ok(map)
    }

    fn parse_sequence(&mut self, indent: usize) -> Result<Vec<YamlValue>, String> {
        let mut items = Vec::new();
        while let Some(line) = self.peek() {
            let line_indent = Self::indent_of(line);
            if line_indent != indent {
                break;
            }
            let trimmed = &line[indent..];
            if !(trimmed.starts_with("- ") || trimmed == "-") {
                break;
            }
            let item_indent = indent + 2;
            let first_content = if trimmed == "-" { "" } else { &trimmed[2..] };
            if first_content.trim().is_empty() {
                self.pos += 1;
                continue;
            }
            if let Some(colon) = find_key_colon(first_content) {
                let key = unquote_scalar(first_content[..colon].trim());
                let rest = first_content[colon + 1..].trim_start();
                self.pos += 1;
                let mut map = OrderedMap::new();
                if rest.is_empty() {
                    map.insert(key, YamlValue::Scalar(String::new()));
                } else {
                    map.insert(key, YamlValue::Scalar(unquote_scalar(rest)));
                }
                let continuation = self.parse_mapping(item_indent)?;
                for (k, v) in continuation.iter() {
                    map.insert(k, v.clone());
                }
                items.push(YamlValue::Mapping(map));
            } else {
                self.pos += 1;
                items.push(YamlValue::Scalar(unquote_scalar(first_content.trim())));
            }
        }
        Ok(items)
    }
}

fn find_key_colon(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_double {
            if c == '\\' {
                i += 1;
            } else if c == '"' {
                in_double = false;
            }
        } else if in_single {
            if c == '\'' {
                if s[i + 1..].starts_with('\'') {
                    i += 1;
                } else {
                    in_single = false;
                }
            }
        } else if c == '"' {
            in_double = true;
        } else if c == '\'' {
            in_single = true;
        } else if c == ':' && (i + 1 == s.len() || bytes[i + 1] == b' ') {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn unquote_scalar(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        return s[1..s.len() - 1].replace("''", "'");
    }
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        return unescape_double_quoted(&s[1..s.len() - 1]);
    }
    s.to_string()
}

fn unescape_double_quoted(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(v) = u32::from_str_radix(&hex, 16) {
                    if let Some(c) = char::from_u32(v) {
                        out.push(c);
                    }
                }
            }
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Ok(v) = u32::from_str_radix(&hex, 16) {
                    if let Some(c) = char::from_u32(v) {
                        out.push(c);
                    }
                }
            }
            Some('U') => {
                let hex: String = chars.by_ref().take(8).collect();
                if let Ok(v) = u32::from_str_radix(&hex, 16) {
                    if let Some(c) = char::from_u32(v) {
                        out.push(c);
                    }
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Parses exactly the restricted grammar `emit()` produces (see module docs). Not a
/// general YAML parser: no flow style beyond empty `[]`/`{}`, no anchors/tags/comments,
/// no multi-document markers.
pub fn parse(text: &str) -> Result<YamlValue, String> {
    let mut p = Parser::new(text);
    let map = p.parse_mapping(0)?;
    Ok(YamlValue::Mapping(map))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(pairs: Vec<(&str, YamlValue)>) -> YamlValue {
        let mut m = OrderedMap::new();
        for (k, v) in pairs {
            m.insert(k, v);
        }
        YamlValue::Mapping(m)
    }

    #[test]
    fn scalar_repr_quoting_rules() {
        assert_eq!(scalar_repr("draft"), "draft");
        assert_eq!(scalar_repr("0.1.0"), "0.1.0");
        assert_eq!(scalar_repr("0.2"), "'0.2'");
        assert_eq!(scalar_repr("2026-08-01T06:41:01Z"), "'2026-08-01T06:41:01Z'");
        assert_eq!(scalar_repr(""), "''");
        assert_eq!(scalar_repr("café"), "\"caf\\xE9\"");
    }

    #[test]
    fn emit_manifest_shape() {
        let value = mapping(vec![
            ("id", YamlValue::Scalar("bundle".into())),
            ("version", YamlValue::Scalar("0.1.0".into())),
            ("title", YamlValue::Scalar("ASCII Markdown Case".into())),
            ("status", YamlValue::Scalar("draft".into())),
            ("created_at", YamlValue::Scalar("2026-08-01T06:41:01Z".into())),
            (
                "sources",
                YamlValue::Sequence(vec![mapping(vec![
                    ("id", YamlValue::Scalar("foo".into())),
                    ("type", YamlValue::Scalar("markdown".into())),
                    ("path", YamlValue::Scalar("sources/originals/foo.md".into())),
                ])]),
            ),
            ("exports", YamlValue::Sequence(vec![])),
            ("okf_version", YamlValue::Scalar("0.2".into())),
        ]);
        let out = emit(&value);
        assert_eq!(
            out,
            "id: bundle\n\
             version: 0.1.0\n\
             title: ASCII Markdown Case\n\
             status: draft\n\
             created_at: '2026-08-01T06:41:01Z'\n\
             sources:\n\
             - id: foo\n  type: markdown\n  path: sources/originals/foo.md\n\
             exports: []\n\
             okf_version: '0.2'\n"
        );
    }

    #[test]
    fn round_trip_manifest_yaml() {
        let text = "id: bundle\n\
                     version: 0.1.0\n\
                     title: GLP-1 Healthcare Reference\n\
                     profile: sop-knowledge-bundle\n\
                     profile_version: 0.2.0\n\
                     status: draft\n\
                     created_at: '2026-08-01T06:41:01Z'\n\
                     updated_at: '2026-08-01T06:41:02Z'\n\
                     sources:\n\
                     - id: follow-up-monitoring-procedure-38ef67176baf\n  type: markdown\n  path: sources/originals/follow-up-monitoring-procedure-38ef67176baf.md\n\
                     - id: primary-care-glp1-sop-a3cca5c9520a\n  type: markdown\n  path: sources/originals/primary-care-glp1-sop-a3cca5c9520a.md\n\
                     exports:\n\
                     - type: graph_json\n  path: ../exports/bundle/graph/graph.json\n\
                     - type: rdf\n  path: ../exports/bundle/graph/triples.ttl\n\
                     okf_version: '0.2'\n";
        let parsed = parse(text).unwrap();
        let re_emitted = emit(&parsed);
        assert_eq!(re_emitted, text);
    }

    // --- 80-column folding: every expected string below is pinned against real
    // `yaml.safe_dump(data, sort_keys=False, allow_unicode=False)` output (Python
    // 3.14 / PyYAML 6.0.3), not hand-derived -- see the Phase 5 session notes.

    #[test]
    fn fold_long_plain_scalar_wraps_at_space_boundary() {
        let value = mapping(vec![(
            "description",
            YamlValue::Scalar(
                "Patients should receive follow-up contact within 14 days after GLP-1 therapy initiation and further evaluation of tolerance."
                    .into(),
            ),
        )]);
        assert_eq!(
            emit(&value),
            "description: Patients should receive follow-up contact within 14 days after GLP-1\n  therapy initiation and further evaluation of tolerance.\n"
        );
    }

    #[test]
    fn fold_single_long_word_does_not_break() {
        let value = mapping(vec![("description", YamlValue::Scalar("a".repeat(120)))]);
        assert_eq!(emit(&value), format!("description: {}\n", "a".repeat(120)));
    }

    #[test]
    fn fold_nested_long_scalar_indents_two_relative_to_key() {
        let mut structured = OrderedMap::new();
        structured.insert(
            "object",
            YamlValue::Scalar(
                "Patients should receive follow-up contact within 14 days after GLP-1 therapy initiation and further care.".into(),
            ),
        );
        let mut stmt = OrderedMap::new();
        stmt.insert("structured_statement", YamlValue::Mapping(structured));
        let value = YamlValue::Mapping({
            let mut m = OrderedMap::new();
            m.insert("sopkb", YamlValue::Mapping(stmt));
            m
        });
        assert_eq!(
            emit(&value),
            "sopkb:\n  structured_statement:\n    object: Patients should receive follow-up contact within 14 days after GLP-1 therapy\n      initiation and further care.\n"
        );
    }

    #[test]
    fn fold_trailing_space_forces_single_quote_and_still_folds() {
        let value = mapping(vec![("description", YamlValue::Scalar("word ".repeat(20)))]);
        assert_eq!(
            emit(&value),
            "description: 'word word word word word word word word word word word word word word\n  word word word word word word '\n"
        );
    }

    #[test]
    fn fold_deeply_nested_scalar_uses_own_indent_plus_two() {
        let mut d = OrderedMap::new();
        d.insert("d", YamlValue::Scalar("word ".repeat(20)));
        let mut c = OrderedMap::new();
        c.insert("c", YamlValue::Mapping(d));
        let mut b = OrderedMap::new();
        b.insert("b", YamlValue::Mapping(c));
        let mut a = OrderedMap::new();
        a.insert("a", YamlValue::Mapping(b));
        assert_eq!(
            emit(&YamlValue::Mapping(a)),
            "a:\n  b:\n    c:\n      d: 'word word word word word word word word word word word word word word word\n        word word word word word '\n"
        );
    }

    #[test]
    fn fold_sequence_scalar_item_indents_two() {
        let value = mapping(vec![("tags", YamlValue::Sequence(vec![YamlValue::Scalar("word ".repeat(20))]))]);
        assert_eq!(
            emit(&value),
            "tags:\n- 'word word word word word word word word word word word word word word word word\n  word word word word '\n"
        );
    }

    #[test]
    fn fold_non_ascii_long_scalar_double_quotes_with_backslash_continuation() {
        let value = mapping(vec![("description", YamlValue::Scalar("café ".repeat(20)))]);
        let expected = "description: \"caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9\\\n  \\ caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9 caf\\xE9\\\n  \\ caf\\xE9 \"\n";
        assert_eq!(emit(&value), expected);
    }

    #[test]
    fn fold_short_scalar_unaffected() {
        let value = mapping(vec![("title", YamlValue::Scalar("Page 1".into()))]);
        assert_eq!(emit(&value), "title: Page 1\n");
    }

    #[test]
    fn raw_values_never_quoted_even_though_scalar_would_quote_them() {
        // Pinned against real `yaml.safe_dump({"confidence": 0.82, "rdf_compatible":
        // True, "start_pos": 42, "condition": None}, sort_keys=False)`.
        let value = mapping(vec![
            ("confidence", YamlValue::Raw("0.82".into())),
            ("rdf_compatible", YamlValue::Raw("true".into())),
            ("start_pos", YamlValue::Raw("42".into())),
            ("condition", YamlValue::Raw("null".into())),
        ]);
        assert_eq!(emit(&value), "confidence: 0.82\nrdf_compatible: true\nstart_pos: 42\ncondition: null\n");
        // The equivalent Scalar (string) forms WOULD be quoted -- proving Raw actually
        // bypasses the heuristic rather than coincidentally matching it.
        let quoted = mapping(vec![("confidence", YamlValue::Scalar("0.82".into()))]);
        assert_eq!(emit(&quoted), "confidence: '0.82'\n");
    }

    #[test]
    fn raw_values_render_bare_inside_a_sequence_too() {
        let value = mapping(vec![("flags", YamlValue::Sequence(vec![YamlValue::Raw("false".into()), YamlValue::Raw("null".into()), YamlValue::Raw("7".into())]))]);
        assert_eq!(emit(&value), "flags:\n- false\n- null\n- 7\n");
    }
}
