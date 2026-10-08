//! Port of `kl4a/kl4a/okf_writer.py`. Grounded via `tools-code` MCP
//! `code_symbols_get` against `source-kl4a-kl4a-okf-writer-py`.
//!
//! This is also where `OkfDocument`/`OkfDocumentError` live — the type
//! several other batches (`apikb::validate`, `codekb::validate`,
//! `codekb::render`) needed (`OkfDocument.parse(text).validate()`, raising
//! `OkfDocumentError`) but could not locate via MCP under any of the
//! phrasings they tried (`kl4a/kl4a/models.py`, a bare `okf.py` file search,
//! etc.). It turned out to be defined in this same shared module as
//! `write_markdown`, not in a separately-named file, which is why those
//! earlier searches (which all assumed a dedicated "okf" or "models" module)
//! came up empty.

use std::fmt;
use std::fs;
use std::path::Path;

use anyhow::Result;
use serde_json::{Map, Value};

/// Port of `kl4a.kl4a.okf_writer.REQUIRED_FRONTMATTER_KEYS`
/// (`kl4a/kl4a/okf_writer.py:10`): `REQUIRED_FRONTMATTER_KEYS = ('type',)`.
pub const REQUIRED_FRONTMATTER_KEYS: [&str; 1] = ["type"];

/// Port of `kl4a.kl4a.okf_writer.OkfDocumentError` (`kl4a/kl4a/okf_writer.py:14-15`).
///
/// ```python
/// class OkfDocumentError(ValueError):
///     pass
/// ```
#[derive(Debug, Clone)]
pub struct OkfDocumentError(pub String);

impl fmt::Display for OkfDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for OkfDocumentError {}

impl OkfDocumentError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

/// Port of `kl4a.kl4a.okf_writer.OkfDocument` (`kl4a/kl4a/okf_writer.py:19-65`).
///
/// ```python
/// @dataclass
/// class OkfDocument:
///     frontmatter: dict[str, Any] = field(default_factory=dict)
///     body: str = ""
/// ```
#[derive(Debug, Clone, Default)]
pub struct OkfDocument {
    pub frontmatter: Map<String, Value>,
    pub body: String,
}

impl OkfDocument {
    /// Port of `OkfDocument.parse` (`kl4a/kl4a/okf_writer.py:24-43`).
    ///
    /// ```python
    /// @classmethod
    /// def parse(cls, text: str) -> OkfDocument:
    ///     lines = text.splitlines()
    ///     if not lines or lines[0].strip() != "---":
    ///         return cls(frontmatter={}, body=text)
    ///     end_idx = None
    ///     for idx in range(1, len(lines)):
    ///         if lines[idx].strip() == "---":
    ///             end_idx = idx
    ///             break
    ///     if end_idx is None:
    ///         raise OkfDocumentError("unterminated YAML frontmatter")
    ///     try:
    ///         frontmatter = yaml.safe_load("\n".join(lines[1:end_idx])) or {}
    ///     except yaml.YAMLError as exc:
    ///         raise OkfDocumentError(f"invalid YAML frontmatter: {exc}") from exc
    ///     if not isinstance(frontmatter, dict):
    ///         raise OkfDocumentError("frontmatter must be a mapping")
    ///     return cls(
    ///         frontmatter=frontmatter, body="\n".join(lines[end_idx + 1 :]).lstrip("\n")
    ///     )
    /// ```
    ///
    /// Python's `str.splitlines()` splits on a broader set of line
    /// terminators than Rust's `str::lines()` (which splits on `\n`/`\r\n`);
    /// treated as an acceptable approximation since OKF markdown files are
    /// written with `\n` (see `write_markdown` below), not `\r`/`\v`/etc.
    pub fn parse(text: &str) -> Result<OkfDocument, OkfDocumentError> {
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() || lines[0].trim() != "---" {
            return Ok(OkfDocument {
                frontmatter: Map::new(),
                body: text.to_string(),
            });
        }
        let mut end_idx: Option<usize> = None;
        for (idx, line) in lines.iter().enumerate().skip(1) {
            if line.trim() == "---" {
                end_idx = Some(idx);
                break;
            }
        }
        let end_idx = end_idx.ok_or_else(|| OkfDocumentError::new("unterminated YAML frontmatter"))?;
        let frontmatter_text = lines[1..end_idx].join("\n");
        let frontmatter_value: Value = if frontmatter_text.trim().is_empty() {
            Value::Object(Map::new())
        } else {
            serde_yaml::from_str(&frontmatter_text)
                .map_err(|exc| OkfDocumentError::new(format!("invalid YAML frontmatter: {exc}")))?
        };
        let frontmatter = match frontmatter_value {
            Value::Null => Map::new(),
            Value::Object(map) => map,
            _ => return Err(OkfDocumentError::new("frontmatter must be a mapping")),
        };
        let body = lines[end_idx + 1..].join("\n");
        let body = body.trim_start_matches('\n').to_string();
        Ok(OkfDocument { frontmatter, body })
    }

    /// Port of `OkfDocument.validate` (`kl4a/kl4a/okf_writer.py:53-65`).
    pub fn validate(&self) -> Result<(), OkfDocumentError> {
        for key in REQUIRED_FRONTMATTER_KEYS {
            let truthy = self
                .frontmatter
                .get(key)
                .map(is_truthy)
                .unwrap_or(false);
            if !truthy {
                return Err(OkfDocumentError::new(format!(
                    "missing required frontmatter key: {key}"
                )));
            }
        }
        let sources = self.frontmatter.get("sources");
        if let Some(sources_val) = sources {
            if is_truthy(sources_val) {
                let arr = sources_val
                    .as_array()
                    .ok_or_else(|| OkfDocumentError::new("sources must be a list when present"))?;
                for source in arr {
                    let ok = source
                        .as_object()
                        .map(|o| o.get("resource").map(is_truthy).unwrap_or(false))
                        .unwrap_or(false);
                    if !ok {
                        return Err(OkfDocumentError::new(
                            "each sources entry must include resource",
                        ));
                    }
                }
            }
        }
        let rule = self
            .frontmatter
            .get("sopkb")
            .and_then(|v| v.as_object())
            .and_then(|o| o.get("rule"));
        if let Some(rule) = rule {
            if !rule.is_null() {
                validate_sopkb_rule(rule)?;
            }
        }
        Ok(())
    }

    /// Port of `OkfDocument.serialize` (`kl4a/kl4a/okf_writer.py:45-51`).
    pub fn serialize(&self) -> Result<String, OkfDocumentError> {
        self.validate()?;
        let frontmatter_value = Value::Object(self.frontmatter.clone());
        let frontmatter_text = serde_yaml::to_string(&frontmatter_value)
            .map_err(|exc| OkfDocumentError::new(format!("serializing frontmatter: {exc}")))?;
        let frontmatter_text = frontmatter_text.trim_end();
        let body = if self.body.ends_with('\n') {
            self.body.clone()
        } else {
            format!("{}\n", self.body)
        };
        Ok(format!("---\n{frontmatter_text}\n---\n\n{body}"))
    }
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Port of `kl4a.kl4a.okf_writer.validate_sopkb_rule` (`kl4a/kl4a/okf_writer.py:68-95`).
pub fn validate_sopkb_rule(rule: &Value) -> Result<(), OkfDocumentError> {
    let rule_obj = rule
        .as_object()
        .ok_or_else(|| OkfDocumentError::new("sopkb.rule must be a mapping"))?;
    if !rule_obj.get("id").map(is_truthy).unwrap_or(false) {
        return Err(OkfDocumentError::new("sopkb.rule.id is required"));
    }
    let obligation = rule_obj
        .get("obligation")
        .and_then(|v| v.as_object())
        .ok_or_else(|| OkfDocumentError::new("sopkb.rule.obligation is required"))?;
    for key in ["fact", "action", "label"] {
        if !obligation.get(key).map(is_truthy).unwrap_or(false) {
            return Err(OkfDocumentError::new(format!(
                "sopkb.rule.obligation.{key} is required"
            )));
        }
    }
    if let Some(condition) = rule_obj.get("condition") {
        if !condition.is_null() {
            let condition_obj = condition
                .as_object()
                .ok_or_else(|| OkfDocumentError::new("sopkb.rule.condition must be a mapping"))?;
            for key in ["fact", "operator", "label"] {
                if !condition_obj.get(key).map(is_truthy).unwrap_or(false) {
                    return Err(OkfDocumentError::new(format!(
                        "sopkb.rule.condition.{key} is required"
                    )));
                }
            }
        }
    }
    if let Some(otherwise) = rule_obj.get("otherwise") {
        if !otherwise.is_null() {
            let otherwise_obj = otherwise
                .as_object()
                .ok_or_else(|| OkfDocumentError::new("sopkb.rule.otherwise must be a mapping"))?;
            if let Some(action_required) = otherwise_obj.get("action_required") {
                if !action_required.is_boolean() {
                    return Err(OkfDocumentError::new(
                        "sopkb.rule.otherwise.action_required must be boolean",
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Port of `kl4a.kl4a.okf_writer.write_markdown` (`kl4a/kl4a/okf_writer.py:134-150`).
///
/// ```python
/// def write_markdown(path: Path, frontmatter: dict[str, Any], body: str) -> None:
///     path.parent.mkdir(parents=True, exist_ok=True)
///     text = (
///         "---\n"
///         + yaml.safe_dump(frontmatter, sort_keys=False, allow_unicode=False).rstrip()
///         + "\n---\n\n"
///     )
///     text += body if body.endswith("\n") else f"{body}\n"
///     path.write_text(text, encoding="utf-8")
/// ```
///
/// NOTE: not atomic in the Python original (plain `write_text`), so this is
/// not atomic either — these are generated docs re-derived from state on
/// every mining run, not state itself.
///
/// `serde_yaml::to_string` preserves map insertion order like
/// `sort_keys=False`, but has no `allow_unicode=False` equivalent —
/// non-ASCII scalars come out as literal UTF-8 rather than escaped.
/// UNCONFIRMED / not bit-for-bit on that axis.
pub fn write_markdown(path: &Path, frontmatter: &Value, body: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let front = serde_yaml::to_string(frontmatter)?;
    let mut text = String::new();
    text.push_str("---\n");
    text.push_str(front.trim_end());
    text.push_str("\n---\n\n");
    if body.ends_with('\n') {
        text.push_str(body);
    } else {
        text.push_str(body);
        text.push('\n');
    }
    fs::write(path, text)?;
    Ok(())
}
