//! `response_text` for both call paths. Shared extraction logic (`output_text` then
//! `output[].content[]` chunk-walking) is identical between the two, and so is the
//! `status == "incomplete"` guard (P-M13, closed 2026-08-22 -- see
//! `docs/port/CATCHUP_PLAN.md`; originally ported as an intentional asymmetry per
//! docs/port/port-mapping-b-mining-settings.md §3.2, but a real mining run against a
//! real Azure profile hit the exact confusing downstream error this guard exists to
//! prevent, twice, so it's no longer treated as an acceptable gap). The two paths still
//! differ in their "unexpected shape" wording -- "response shape" (author) vs
//! "responses shape" (chat) -- both verbatim from their respective pseudocode, not a
//! typo to fix.

use crate::pyrepr::python_repr;
use serde_json::Value;
use sopkb_core::error::{Result, SopkbError};

/// `output_text` if non-blank (returned UNSTRIPPED), else the joined text of
/// `output[].content[]` parts whose `type` is `"output_text"` or `"text"`, else
/// `None` (caller raises the shape-specific error).
pub(crate) fn extract_output_text_or_chunks(body: &Value) -> Option<String> {
    if let Some(Value::String(s)) = body.get("output_text") {
        if !s.trim().is_empty() {
            return Some(s.clone());
        }
    }
    let mut chunks = Vec::new();
    if let Some(items) = body.get("output").and_then(|v| v.as_array()) {
        for item in items {
            if !item.is_object() {
                continue;
            }
            if let Some(parts) = item.get("content").and_then(|v| v.as_array()) {
                for part in parts {
                    if !part.is_object() {
                        continue;
                    }
                    let ty = part.get("type").and_then(|v| v.as_str());
                    if matches!(ty, Some("output_text") | Some("text")) {
                        if let Some(Value::String(text)) = part.get("text") {
                            chunks.push(text.clone());
                        }
                    }
                }
            }
        }
    }
    if chunks.is_empty() {
        None
    } else {
        Some(chunks.join("\n"))
    }
}

/// Python `str()` of a JSON-sourced scalar (used for f-string interpolation, as
/// opposed to `repr()`): unquoted strings, `None`/`True`/`False`, bare numbers.
fn py_display(v: &Value) -> String {
    match v {
        Value::Null => "None".to_string(),
        Value::Bool(b) => {
            if *b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                n.to_string()
            } else if let Some(f) = n.as_f64() {
                sopkb_fmt::py_float(f)
            } else {
                n.to_string()
            }
        }
        other => python_repr(other),
    }
}

/// `usage.get(field)` -- missing/null renders as the literal `"None"` in the message.
fn usage_field(body: &Value, field: &str) -> String {
    match body.get("usage").and_then(|u| u.get(field)) {
        None | Some(Value::Null) => "None".to_string(),
        Some(v) => py_display(v),
    }
}

/// `status == "incomplete"` guard shared by both call paths (P-M13): a truncated
/// response is always an error, even when partial-but-parseable text exists, since
/// accepting it silently just relocates the failure to a much more confusing place
/// downstream (an "unterminated string" JSON error, or a "missing required frontmatter
/// key" validation error with no indication either was ever about truncation).
fn incomplete_status_error(body: &Value) -> Option<SopkbError> {
    if body.get("status").and_then(|v| v.as_str()) != Some("incomplete") {
        return None;
    }
    let reason = body
        .get("incomplete_details")
        .and_then(|d| d.get("reason"))
        .and_then(|r| r.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let total_tokens = usage_field(body, "total_tokens");
    let output_tokens = usage_field(body, "output_tokens");
    Some(SopkbError::Value(format!(
        "Azure OpenAI response incomplete (reason={reason}, total_tokens={total_tokens}, \
         output_tokens={output_tokens}). Increase AZURE_OPENAI_MAX_OUTPUT_TOKENS or \
         lower AZURE_OPENAI_REASONING_EFFORT."
    )))
}

/// `okf_author.response_text`, plus the P-M13 incomplete guard (see module doc).
pub(crate) fn response_text_author(body: &Value) -> Result<String> {
    if let Some(err) = incomplete_status_error(body) {
        return Err(err);
    }
    if let Some(text) = extract_output_text_or_chunks(body) {
        return Ok(text);
    }
    Err(SopkbError::Value(format!("Unexpected Azure OpenAI response shape: {}", python_repr(body))))
}

/// `agent_chat.response_text`: `status == "incomplete"` is checked before any text
/// extraction, so a truncated response is always an error even if partial text exists.
pub(crate) fn response_text_chat(body: &Value) -> Result<String> {
    if let Some(err) = incomplete_status_error(body) {
        return Err(err);
    }
    if let Some(text) = extract_output_text_or_chunks(body) {
        return Ok(text);
    }
    Err(SopkbError::Value(format!("Unexpected Azure OpenAI responses shape: {}", python_repr(body))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn output_text_wins_and_is_returned_unstripped() {
        let body = json!({"output_text": "  hello  "});
        assert_eq!(response_text_author(&body).unwrap(), "  hello  ");
        assert_eq!(response_text_chat(&body).unwrap(), "  hello  ");
    }

    #[test]
    fn blank_output_text_falls_through_to_chunks() {
        let body = json!({
            "output_text": "   ",
            "output": [
                {"content": [{"type": "reasoning"}, {"type": "output_text", "text": "chunk1"}]},
                {"content": [{"type": "text", "text": "chunk2"}]},
            ]
        });
        assert_eq!(response_text_author(&body).unwrap(), "chunk1\nchunk2");
    }

    #[test]
    fn neither_present_raises_shape_error_with_distinct_wording() {
        let body = json!({"weird": true});
        let author_err = response_text_author(&body).unwrap_err().to_string();
        assert!(author_err.starts_with("Unexpected Azure OpenAI response shape: "));
        let chat_err = response_text_chat(&body).unwrap_err().to_string();
        assert!(chat_err.starts_with("Unexpected Azure OpenAI responses shape: "));
    }

    #[test]
    fn chat_incomplete_status_short_circuits_before_extraction() {
        let body = json!({
            "status": "incomplete",
            "output_text": "partial but discarded",
            "incomplete_details": {"reason": "max_output_tokens"},
            "usage": {"total_tokens": 500, "output_tokens": 400},
        });
        let err = response_text_chat(&body).unwrap_err().to_string();
        assert_eq!(
            err,
            "Azure OpenAI response incomplete (reason=max_output_tokens, total_tokens=500, \
             output_tokens=400). Increase AZURE_OPENAI_MAX_OUTPUT_TOKENS or lower \
             AZURE_OPENAI_REASONING_EFFORT."
        );
    }

    #[test]
    fn chat_incomplete_missing_usage_and_reason_render_as_unknown_and_none() {
        let body = json!({"status": "incomplete"});
        let err = response_text_chat(&body).unwrap_err().to_string();
        assert_eq!(
            err,
            "Azure OpenAI response incomplete (reason=unknown, total_tokens=None, \
             output_tokens=None). Increase AZURE_OPENAI_MAX_OUTPUT_TOKENS or lower \
             AZURE_OPENAI_REASONING_EFFORT."
        );
    }

    #[test]
    fn author_path_now_rejects_incomplete_status_too() {
        // P-M13 closed 2026-08-22: the author path used to ignore `status` entirely and
        // accept `output_text` from a truncated response. It now errors the same way
        // the chat path always has.
        let body = json!({
            "status": "incomplete",
            "output_text": "still present but truncated",
            "incomplete_details": {"reason": "max_output_tokens"},
            "usage": {"total_tokens": 16000, "output_tokens": 15990},
        });
        let err = response_text_author(&body).unwrap_err().to_string();
        assert_eq!(
            err,
            "Azure OpenAI response incomplete (reason=max_output_tokens, total_tokens=16000, \
             output_tokens=15990). Increase AZURE_OPENAI_MAX_OUTPUT_TOKENS or lower \
             AZURE_OPENAI_REASONING_EFFORT."
        );
    }
}
