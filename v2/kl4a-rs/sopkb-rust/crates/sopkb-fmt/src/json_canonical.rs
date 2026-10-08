//! Byte-exact emulation of Python's `json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True)`
//! plus a single trailing newline, which is what `bundle_store.write_json` always uses.
//!
//! docs/port/port-mapping-a-core-data.md §3.3 `write_json`:
//!   - sort_keys=True: every object's keys sorted at every depth by Python code-point order.
//!     `serde_json::Map` (without the `preserve_order` feature) is BTreeMap-backed, and
//!     `str`'s `Ord` is byte-wise UTF-8 order, which is equivalent to code-point order for
//!     valid UTF-8 -- so no explicit sort is needed here, the Map is already in the right order.
//!   - indent=2, item separator ",\n", key separator ": ". Empty containers render inline
//!     as `[]`/`{}` even under indent.
//!   - ensure_ascii=True: any char outside the printable ASCII range 0x20..=0x7E is escaped,
//!     astral characters as UTF-16 surrogate pairs.
//!   - float formatting is Python `repr()` shortest-round-trip (`py_float`).

use crate::py_float::py_float;
use serde_json::Value;
use std::fmt::Write as _;

/// Matches `json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True)`.
/// Does NOT include the trailing newline `write_json` appends -- callers add that.
pub fn to_canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, 0);
    out
}

/// Matches `json.dumps(value, sort_keys=True, ensure_ascii=True)` with NO `indent`
/// argument -- Python's default separators when `indent` is `None` are `(", ", ": ")`
/// (compact, but with a space after each comma/colon, unlike a truly minified
/// `(",", ":")` encoder). This is the wire format `sopkb-mcp`'s JSONL stdio transport
/// uses for its single-line response envelopes (docs/port/port-mapping-d-agent-mcp.md
/// §6.5) -- `to_canonical_json`'s indented, `(",\n", ": ")`-separated output is a
/// different, incompatible shape and must not be used there.
pub fn to_compact_json(value: &Value) -> String {
    let mut out = String::new();
    write_value_compact(&mut out, value);
    out
}

fn indent(depth: usize) -> String {
    "  ".repeat(depth)
}

fn write_value(out: &mut String, value: &Value, depth: usize) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => write_number(out, n),
        Value::String(s) => write_json_string(out, s),
        Value::Array(items) => write_array(out, items, depth),
        Value::Object(map) => write_object(out, map, depth),
    }
}

fn write_array(out: &mut String, items: &[Value], depth: usize) {
    if items.is_empty() {
        out.push_str("[]");
        return;
    }
    out.push_str("[\n");
    let inner = indent(depth + 1);
    for (i, item) in items.iter().enumerate() {
        out.push_str(&inner);
        write_value(out, item, depth + 1);
        if i + 1 < items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&indent(depth));
    out.push(']');
}

fn write_object(out: &mut String, map: &serde_json::Map<String, Value>, depth: usize) {
    if map.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push_str("{\n");
    let inner = indent(depth + 1);
    let len = map.len();
    for (i, (k, v)) in map.iter().enumerate() {
        out.push_str(&inner);
        write_json_string(out, k);
        out.push_str(": ");
        write_value(out, v, depth + 1);
        if i + 1 < len {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&indent(depth));
    out.push('}');
}

fn write_value_compact(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => write_number(out, n),
        Value::String(s) => write_json_string(out, s),
        Value::Array(items) => write_array_compact(out, items),
        Value::Object(map) => write_object_compact(out, map),
    }
}

fn write_array_compact(out: &mut String, items: &[Value]) {
    out.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_value_compact(out, item);
    }
    out.push(']');
}

fn write_object_compact(out: &mut String, map: &serde_json::Map<String, Value>) {
    out.push('{');
    for (i, (k, v)) in map.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_json_string(out, k);
        out.push_str(": ");
        write_value_compact(out, v);
    }
    out.push('}');
}

fn write_number(out: &mut String, n: &serde_json::Number) {
    if let Some(i) = n.as_i64() {
        let _ = write!(out, "{i}");
    } else if let Some(u) = n.as_u64() {
        let _ = write!(out, "{u}");
    } else if let Some(f) = n.as_f64() {
        out.push_str(&py_float(f));
    }
}

/// Escapes exactly as Python's `json.encoder` does with `ensure_ascii=True`:
/// `"`, `\`, the six named control-char escapes, any other char < 0x20 as
/// `\u00XX`, any char outside printable ASCII (0x20..=0x7E) as `\uXXXX`
/// (UTF-16 surrogate pair for astral code points).
fn write_json_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        let cp = ch as u32;
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            _ if cp < 0x20 => {
                let _ = write!(out, "\\u{cp:04x}");
            }
            _ if (0x20..=0x7E).contains(&cp) => out.push(ch),
            _ if cp <= 0xFFFF => {
                let _ = write!(out, "\\u{cp:04x}");
            }
            _ => {
                let v = cp - 0x10000;
                let high = 0xD800 + (v >> 10);
                let low = 0xDC00 + (v & 0x3FF);
                let _ = write!(out, "\\u{high:04x}\\u{low:04x}");
            }
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_containers_render_inline() {
        assert_eq!(to_canonical_json(&json!({})), "{}");
        assert_eq!(to_canonical_json(&json!([])), "[]");
    }

    #[test]
    fn keys_sort_at_every_depth() {
        let v = json!({"b": 1, "a": {"z": 1, "y": 2}});
        assert_eq!(
            to_canonical_json(&v),
            "{\n  \"a\": {\n    \"y\": 2,\n    \"z\": 1\n  },\n  \"b\": 1\n}"
        );
    }

    #[test]
    fn ascii_escaping() {
        let v = json!({"k": "café ☕ 北京"});
        let s = to_canonical_json(&v);
        assert!(s.contains("caf\\u00e9"));
        assert!(s.contains("\\u2615"));
        assert!(s.contains("\\u5317\\u4eac"));
    }

    #[test]
    fn float_formatting() {
        assert_eq!(to_canonical_json(&json!(0.82)), "0.82");
        assert_eq!(to_canonical_json(&json!(1.0)), "1.0");
    }

    #[test]
    fn array_of_objects_matches_python_indent_style() {
        let v = json!([{"id": "a"}, {"id": "b"}]);
        assert_eq!(
            to_canonical_json(&v),
            "[\n  {\n    \"id\": \"a\"\n  },\n  {\n    \"id\": \"b\"\n  }\n]"
        );
    }

    #[test]
    fn compact_empty_containers_render_inline() {
        assert_eq!(to_compact_json(&json!({})), "{}");
        assert_eq!(to_compact_json(&json!([])), "[]");
    }

    #[test]
    fn compact_keys_sort_at_every_depth_with_comma_space_separators() {
        let v = json!({"b": 1, "a": {"z": 1, "y": 2}});
        assert_eq!(to_compact_json(&v), r#"{"a": {"y": 2, "z": 1}, "b": 1}"#);
    }

    #[test]
    fn compact_matches_python_json_dumps_sort_keys_no_indent() {
        // json.dumps({"error": {"code": -32000, "message": "boom"}, "id": 8, "jsonrpc": "2.0"}, sort_keys=True)
        let v = json!({"jsonrpc": "2.0", "id": 8, "error": {"code": -32000, "message": "boom"}});
        assert_eq!(
            to_compact_json(&v),
            r#"{"error": {"code": -32000, "message": "boom"}, "id": 8, "jsonrpc": "2.0"}"#
        );
    }

    #[test]
    fn compact_array_uses_comma_space() {
        assert_eq!(to_compact_json(&json!([1, 2, 3])), "[1, 2, 3]");
    }
}
