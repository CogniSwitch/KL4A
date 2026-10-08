//! An insertion-order-preserving JSON value type, plus a hand-rolled recursive-descent
//! parser that reads directly from raw text.
//!
//! ## Why this exists
//!
//! `serde_json::Value`/`serde_json::Map` in this workspace is deliberately
//! **BTreeMap-backed** (`preserve_order` is NOT enabled -- see the doc comment at the
//! top of `sopkb-fmt::json_canonical`, which relies on that fact to implement
//! `sort_keys=True` for free). That means any JSON parsed via `serde_json::from_str`
//! has its object keys silently re-sorted alphabetically at parse time, before any
//! caller code ever sees the original order.
//!
//! For most of this port that's exactly what's wanted (Python's `json.dumps(...,
//! sort_keys=True)` re-sorts anyway, so losing insertion order is unobservable). It is
//! NOT what's wanted for one specific value: an authored OKF document's `frontmatter`,
//! which `write_concept_doc` serializes via `yaml_safe_dump(frontmatter,
//! sort_keys=False)` -- i.e. the *original* key order (whatever order the LLM happened
//! to emit its JSON object in) must survive all the way into the YAML file on disk.
//!
//! Since order is lost the instant `serde_json::from_str` builds a `Map`, and there is
//! no way to parse "everything as a BTreeMap except this one subtree", the entire
//! author-response body is parsed with this module's own parser instead of
//! `serde_json`. Every other field in the response (`concept_id`, `body`,
//! `knowledge_items[].*`, `metadata`, `decision_rules`, ...) is happily converted down
//! to a plain `serde_json::Value` once extracted (order genuinely doesn't matter for
//! those -- they either get individually inspected as scalars, or land in a
//! `write_json`/`to_canonical_json` output that re-sorts anyway); only `frontmatter`
//! is threaded through as `OrderedJson` end-to-end, from parse to
//! `sopkb_fmt::YamlValue` to `emit_yaml`.
//!
//! This is intentionally NOT a general-purpose JSON library: it implements the full
//! JSON grammar (objects, arrays, strings with `\uXXXX`/surrogate-pair escapes,
//! numbers, `true`/`false`/`null`) because a real LLM response is untrusted,
//! arbitrary-shaped JSON text, but it makes no attempt at helpful error recovery
//! (no fence-stripping, no trailing-comma tolerance) -- that tolerance is applied by
//! the caller (`okf_author::parse_author_response`) before the raw text reaches this
//! parser, matching the P-M12 fence-stripping fix's scope.

use sopkb_fmt::OrderedMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum OrderedJson {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<OrderedJson>),
    Object(OrderedMap<OrderedJson>),
}

impl OrderedJson {
    /// Python truthiness for a JSON-sourced value: `None`/`False`/`0`/`0.0`/`""`/
    /// `[]`/`{}` are all falsy; everything else (including a non-empty string that is
    /// literally `"0"`, and any nonzero number) is truthy.
    pub fn is_truthy(&self) -> bool {
        match self {
            OrderedJson::Null => false,
            OrderedJson::Bool(b) => *b,
            OrderedJson::Number(n) => {
                if let Some(i) = n.as_i64() {
                    i != 0
                } else if let Some(u) = n.as_u64() {
                    u != 0
                } else {
                    n.as_f64().map(|f| f != 0.0).unwrap_or(true)
                }
            }
            OrderedJson::String(s) => !s.is_empty(),
            OrderedJson::Array(a) => !a.is_empty(),
            OrderedJson::Object(o) => !o.is_empty(),
        }
    }

    pub fn as_object(&self) -> Option<&OrderedMap<OrderedJson>> {
        match self {
            OrderedJson::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[OrderedJson]> {
        match self {
            OrderedJson::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            OrderedJson::String(s) => Some(s),
            _ => None,
        }
    }

    /// Converts to a (order-losing) `serde_json::Value`. Safe for anything except a
    /// `frontmatter` object destined for YAML emission -- see the module doc comment.
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            OrderedJson::Null => serde_json::Value::Null,
            OrderedJson::Bool(b) => serde_json::Value::Bool(*b),
            OrderedJson::Number(n) => serde_json::Value::Number(n.clone()),
            OrderedJson::String(s) => serde_json::Value::String(s.clone()),
            OrderedJson::Array(items) => serde_json::Value::Array(items.iter().map(|v| v.to_json_value()).collect()),
            OrderedJson::Object(map) => {
                let mut m = serde_json::Map::new();
                for (k, v) in map.iter() {
                    m.insert(k.to_string(), v.to_json_value());
                }
                serde_json::Value::Object(m)
            }
        }
    }

    /// Converts an `Object` variant's entries to a (order-losing) `serde_json::Map`.
    /// Returns an empty map for any non-object variant.
    pub fn to_json_map(&self) -> serde_json::Map<String, serde_json::Value> {
        match self {
            OrderedJson::Object(map) => {
                let mut m = serde_json::Map::new();
                for (k, v) in map.iter() {
                    m.insert(k.to_string(), v.to_json_value());
                }
                m
            }
            _ => serde_json::Map::new(),
        }
    }

    /// Python `str(x)` coercion for a JSON-sourced scalar: strings pass through
    /// unquoted, booleans render as `True`/`False`, `None` as `None`, numbers as their
    /// Python `repr`. Used for `item_type`/`review_status` fields, which Python
    /// coerces with `str(candidate.get(...) or default)`.
    pub fn py_str(&self) -> String {
        match self {
            OrderedJson::Null => "None".to_string(),
            OrderedJson::Bool(b) => {
                if *b {
                    "True".to_string()
                } else {
                    "False".to_string()
                }
            }
            OrderedJson::String(s) => s.clone(),
            OrderedJson::Number(n) => {
                if let Some(i) = n.as_i64() {
                    i.to_string()
                } else if let Some(u) = n.as_u64() {
                    u.to_string()
                } else if let Some(f) = n.as_f64() {
                    sopkb_fmt::py_float(f)
                } else {
                    n.to_string()
                }
            }
            // Unreachable in practice (item_type/review_status are documented as short
            // strings in the LLM schema); a best-effort fallback avoids a panic rather
            // than trying to reproduce Python's list/dict repr exactly.
            other => format!("{other:?}"),
        }
    }

    /// Python `float(x)` coercion, used for `confidence`.
    pub fn py_float_of(&self) -> Result<f64, String> {
        match self {
            OrderedJson::Number(n) => Ok(n.as_f64().unwrap_or(0.0)),
            OrderedJson::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            OrderedJson::String(s) => {
                s.trim().parse::<f64>().map_err(|_| format!("could not convert string to float: '{s}'"))
            }
            other => Err(format!("float() argument must be a string or a number, not {other:?}")),
        }
    }
}

/// Converts `OrderedJson` to `sopkb_fmt::YamlValue`, preserving object key order
/// end-to-end. Booleans/numbers/null become a pre-formatted `YamlValue::Raw` token
/// (rendered bare/unquoted by `emit_yaml`), never `Scalar`'s string-quoting heuristic
/// meant for actual Python strings -- `YamlValue` itself has no typed bool/int/float/
/// null variants (see `sopkb-fmt/src/yaml_pyemit.rs`'s `Raw` doc comment).
pub fn ordered_json_to_yaml(value: &OrderedJson) -> sopkb_fmt::YamlValue {
    use sopkb_fmt::YamlValue;
    match value {
        OrderedJson::Null => YamlValue::Raw("null".to_string()),
        OrderedJson::Bool(b) => YamlValue::Raw(if *b { "true" } else { "false" }.to_string()),
        OrderedJson::Number(n) => {
            if let Some(i) = n.as_i64() {
                YamlValue::Raw(i.to_string())
            } else if let Some(u) = n.as_u64() {
                YamlValue::Raw(u.to_string())
            } else {
                YamlValue::Raw(sopkb_fmt::py_float(n.as_f64().unwrap_or(0.0)))
            }
        }
        OrderedJson::String(s) => YamlValue::Scalar(s.clone()),
        OrderedJson::Array(items) => YamlValue::Sequence(items.iter().map(ordered_json_to_yaml).collect()),
        OrderedJson::Object(map) => {
            let mut m = OrderedMap::new();
            for (k, v) in map.iter() {
                m.insert(k, ordered_json_to_yaml(v));
            }
            YamlValue::Mapping(m)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonParseError(pub String);

impl fmt::Display for JsonParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for JsonParseError {}

/// Parses `text` as a single JSON value, preserving object key insertion order.
/// Implements the full JSON grammar; trailing non-whitespace content after the value
/// is an error (matches `json.loads`, which also rejects trailing garbage).
pub fn parse_ordered_json(text: &str) -> Result<OrderedJson, JsonParseError> {
    let mut p = JParser { chars: text.chars().collect(), pos: 0 };
    p.skip_ws();
    let v = p.parse_value()?;
    p.skip_ws();
    if p.pos != p.chars.len() {
        return Err(JsonParseError(format!("unexpected trailing content at position {}", p.pos)));
    }
    Ok(v)
}

struct JParser {
    chars: Vec<char>,
    pos: usize,
}

impl JParser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
    }

    fn expect_char(&mut self, c: char) -> Result<(), JsonParseError> {
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(JsonParseError(format!("expected '{c}' at position {}", self.pos)))
        }
    }

    fn expect_literal(&mut self, lit: &str, value: OrderedJson) -> Result<OrderedJson, JsonParseError> {
        for expected in lit.chars() {
            if self.bump() != Some(expected) {
                return Err(JsonParseError(format!("invalid literal at position {}", self.pos)));
            }
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<OrderedJson, JsonParseError> {
        self.skip_ws();
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => self.parse_string().map(OrderedJson::String),
            Some('t') => self.expect_literal("true", OrderedJson::Bool(true)),
            Some('f') => self.expect_literal("false", OrderedJson::Bool(false)),
            Some('n') => self.expect_literal("null", OrderedJson::Null),
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(JsonParseError(format!("unexpected character '{c}' at position {}", self.pos))),
            None => Err(JsonParseError("unexpected end of input, expected a value".to_string())),
        }
    }

    fn parse_object(&mut self) -> Result<OrderedJson, JsonParseError> {
        self.expect_char('{')?;
        let mut map = OrderedMap::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(OrderedJson::Object(map));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some('"') {
                return Err(JsonParseError(format!("expected string key at position {}", self.pos)));
            }
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect_char(':')?;
            let value = self.parse_value()?;
            map.insert(key, value);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                }
                Some('}') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(JsonParseError(format!("expected ',' or '}}' at position {}", self.pos))),
            }
        }
        Ok(OrderedJson::Object(map))
    }

    fn parse_array(&mut self) -> Result<OrderedJson, JsonParseError> {
        self.expect_char('[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(OrderedJson::Array(items));
        }
        loop {
            let value = self.parse_value()?;
            items.push(value);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                }
                Some(']') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(JsonParseError(format!("expected ',' or ']' at position {}", self.pos))),
            }
        }
        Ok(OrderedJson::Array(items))
    }

    fn parse_string(&mut self) -> Result<String, JsonParseError> {
        self.expect_char('"')?;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err(JsonParseError("unterminated string".to_string())),
                Some('"') => break,
                Some('\\') => match self.bump() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('b') => out.push('\u{08}'),
                    Some('f') => out.push('\u{0C}'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('u') => {
                        let cp = self.parse_hex4()?;
                        if (0xD800..=0xDBFF).contains(&cp) {
                            // High surrogate: expect a following \uDCxx low surrogate.
                            if self.bump() != Some('\\') || self.bump() != Some('u') {
                                return Err(JsonParseError("expected low surrogate escape".to_string()));
                            }
                            let low = self.parse_hex4()?;
                            if !(0xDC00..=0xDFFF).contains(&low) {
                                return Err(JsonParseError("invalid low surrogate".to_string()));
                            }
                            let combined = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                            match char::from_u32(combined) {
                                Some(c) => out.push(c),
                                None => return Err(JsonParseError("invalid surrogate pair".to_string())),
                            }
                        } else {
                            match char::from_u32(cp) {
                                Some(c) => out.push(c),
                                None => return Err(JsonParseError("invalid \\u escape".to_string())),
                            }
                        }
                    }
                    _ => return Err(JsonParseError(format!("invalid escape at position {}", self.pos))),
                },
                Some(c) => out.push(c),
            }
        }
        Ok(out)
    }

    fn parse_hex4(&mut self) -> Result<u32, JsonParseError> {
        let mut v: u32 = 0;
        for _ in 0..4 {
            let c = self.bump().ok_or_else(|| JsonParseError("truncated \\u escape".to_string()))?;
            let digit = c.to_digit(16).ok_or_else(|| JsonParseError(format!("invalid hex digit '{c}'")))?;
            v = v * 16 + digit;
        }
        Ok(v)
    }

    fn parse_number(&mut self) -> Result<OrderedJson, JsonParseError> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
        }
        if self.peek() == Some('.') {
            self.pos += 1;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some('+' | '-')) {
                self.pos += 1;
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        if text.is_empty() || text == "-" {
            return Err(JsonParseError(format!("invalid number at position {start}")));
        }
        // Delegate to serde_json's own (already-correct) number parsing rather than
        // reimplementing int/float distinction and overflow handling by hand.
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| JsonParseError(format!("invalid number '{text}': {e}")))?;
        match v {
            serde_json::Value::Number(n) => Ok(OrderedJson::Number(n)),
            _ => Err(JsonParseError(format!("invalid number '{text}'"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars() {
        assert_eq!(parse_ordered_json("true").unwrap(), OrderedJson::Bool(true));
        assert_eq!(parse_ordered_json("false").unwrap(), OrderedJson::Bool(false));
        assert_eq!(parse_ordered_json("null").unwrap(), OrderedJson::Null);
        assert_eq!(parse_ordered_json("42").unwrap().as_str(), None);
        assert_eq!(parse_ordered_json("\"hi\"").unwrap(), OrderedJson::String("hi".to_string()));
    }

    #[test]
    fn parses_number_variants() {
        let OrderedJson::Number(n) = parse_ordered_json("-3.5e2").unwrap() else { panic!("expected number") };
        assert_eq!(n.as_f64(), Some(-350.0));
        let OrderedJson::Number(n) = parse_ordered_json("7").unwrap() else { panic!("expected number") };
        assert_eq!(n.as_i64(), Some(7));
    }

    #[test]
    fn preserves_object_key_order_even_when_alphabetically_reversed() {
        let parsed = parse_ordered_json(r#"{"zebra": 1, "apple": 2, "mango": 3}"#).unwrap();
        let obj = parsed.as_object().unwrap();
        let keys: Vec<&str> = obj.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["zebra", "apple", "mango"]);
    }

    #[test]
    fn parses_nested_structures_and_escapes() {
        let parsed = parse_ordered_json(r#"{"a": [1, "b\nc", {"d": null}], "e": true}"#).unwrap();
        let obj = parsed.as_object().unwrap();
        let arr = obj.get("a").unwrap().as_array().unwrap();
        assert_eq!(arr[1], OrderedJson::String("b\nc".to_string()));
        assert_eq!(arr[2].as_object().unwrap().get("d"), Some(&OrderedJson::Null));
        assert_eq!(obj.get("e"), Some(&OrderedJson::Bool(true)));
    }

    #[test]
    fn parses_unicode_escape_and_surrogate_pair() {
        let parsed = parse_ordered_json(r#""café""#).unwrap();
        assert_eq!(parsed, OrderedJson::String("café".to_string()));
        // U+1F600 (grinning face) as a UTF-16 surrogate pair.
        let parsed = parse_ordered_json(r#""😀""#).unwrap();
        assert_eq!(parsed, OrderedJson::String("\u{1F600}".to_string()));
    }

    #[test]
    fn rejects_trailing_garbage_and_malformed_input() {
        assert!(parse_ordered_json("{}garbage").is_err());
        assert!(parse_ordered_json("{\"a\":}").is_err());
        assert!(parse_ordered_json("").is_err());
    }

    #[test]
    fn truthiness_matches_python() {
        assert!(!OrderedJson::Null.is_truthy());
        assert!(!OrderedJson::Bool(false).is_truthy());
        assert!(!OrderedJson::String(String::new()).is_truthy());
        assert!(!OrderedJson::Array(vec![]).is_truthy());
        assert!(!OrderedJson::Object(OrderedMap::new()).is_truthy());
        assert!(OrderedJson::String("0".to_string()).is_truthy());
        assert!(OrderedJson::Bool(true).is_truthy());
    }
}
