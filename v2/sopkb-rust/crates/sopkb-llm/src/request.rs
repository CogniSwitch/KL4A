//! Shared request-building pieces: the outbound URL, `int()`-emulation for the two
//! numeric settings fields, the compact (non-canonical) request body serializer, and
//! the POST-and-parse call with HTTP-error wrapping.

use crate::transport::{HttpRequest, Transport};
use serde_json::Value;
use sopkb_core::error::{Result, SopkbError};

/// `rstrip_all_trailing_slashes(base_url) + "/responses"` -- Python's `.rstrip('/')`
/// removes every trailing `/`, nothing else (no scheme validation, no
/// percent-encoding, query strings preserved as-is).
pub fn azure_responses_url(profile_id: Option<&str>) -> Result<String> {
    let base_url = sopkb_config::require("base_url", "an LLM API base URL", profile_id)?;
    Ok(format!("{}/responses", base_url.trim_end_matches('/')))
}

/// `int(s)` for the two numeric settings fields this crate parses
/// (`max_output_tokens`, `timeout_seconds`). Both are always plain decimal-digit
/// strings in practice (defaults `"4096"`/`"60"`, or user-entered profile values), so
/// a full arbitrary-base Python `int()` emulation (underscores, leading `+`,
/// `0x`/`0o`/`0b` rejection nuance) isn't attempted; the error message shape mirrors
/// Python's raw (unfriendly, uncaught) `ValueError` text.
pub fn parse_py_int(s: &str) -> Result<i64> {
    s.parse::<i64>().map_err(|_| SopkbError::Value(format!("invalid literal for int() with base 10: '{s}'")))
}

fn json_string(s: &str) -> String {
    sopkb_fmt::to_canonical_json(&Value::String(s.to_string()))
}

/// `json_dumps(body)` with Python's default (non-indent) separators `", "`/`": "`,
/// preserving the exact key insertion order `model, instructions, input,
/// max_output_tokens` -- `okf_author.call_azure_responses` never adds a `reasoning` key.
pub fn compact_body_no_reasoning(model: &str, instructions: &str, input: &str, max_output_tokens: i64) -> String {
    format!(
        "{{\"model\": {}, \"instructions\": {}, \"input\": {}, \"max_output_tokens\": {}}}",
        json_string(model),
        json_string(instructions),
        json_string(input),
        max_output_tokens
    )
}

/// Same shape, plus a trailing `"reasoning": {"effort": ...}` key when
/// `reasoning_effort` is non-empty -- `agent_chat.call_azure_responses` only.
pub fn compact_body_with_reasoning(model: &str, instructions: &str, input: &str, max_output_tokens: i64, reasoning_effort: Option<&str>) -> String {
    match reasoning_effort {
        Some(effort) if !effort.is_empty() => format!(
            "{{\"model\": {}, \"instructions\": {}, \"input\": {}, \"max_output_tokens\": {}, \"reasoning\": {{\"effort\": {}}}}}",
            json_string(model),
            json_string(instructions),
            json_string(input),
            max_output_tokens,
            json_string(effort)
        ),
        _ => compact_body_no_reasoning(model, instructions, input, max_output_tokens),
    }
}

/// POSTs and parses the JSON response body. A non-2xx status is wrapped as
/// `"Azure OpenAI {failed_label} request failed: HTTP {status}: {error_text}"`
/// (matching each call path's exact prefix -- "author" vs "agent"); a connection-level
/// failure propagates the raw transport message unwrapped, matching "only HTTPError is
/// caught" in both pseudocode originals. A non-JSON response body on a 2xx status also
/// propagates unwrapped (Python's `json_parse` failure is not caught by `except
/// HTTPError` either).
pub fn post_and_parse(transport: &dyn Transport, url: String, headers: Vec<(String, String)>, body: Vec<u8>, timeout_secs: u64, failed_label: &str) -> Result<Value> {
    let request = HttpRequest { url, headers, body, timeout_secs };
    let response = transport.post_json(request).map_err(|e| SopkbError::Value(e.0))?;
    if response.status >= 400 {
        let error_text = String::from_utf8_lossy(&response.body).into_owned();
        return Err(SopkbError::Value(format!("Azure OpenAI {failed_label} request failed: HTTP {}: {error_text}", response.status)));
    }
    let text = String::from_utf8_lossy(&response.body).into_owned();
    serde_json::from_str(&text).map_err(|e| SopkbError::Value(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_py_int_accepts_digits_rejects_garbage() {
        assert_eq!(parse_py_int("4096").unwrap(), 4096);
        let err = parse_py_int("not-a-number").unwrap_err().to_string();
        assert_eq!(err, "invalid literal for int() with base 10: 'not-a-number'");
    }

    #[test]
    fn compact_body_key_order_and_no_reasoning_key() {
        let body = compact_body_no_reasoning("gpt-x", "sys prompt", "user input", 4096);
        assert_eq!(body, r#"{"model": "gpt-x", "instructions": "sys prompt", "input": "user input", "max_output_tokens": 4096}"#);
        assert!(!body.contains("reasoning"));
    }

    #[test]
    fn compact_body_with_reasoning_appends_key_only_when_non_empty() {
        let with_effort = compact_body_with_reasoning("gpt-x", "sys", "in", 4096, Some("high"));
        assert_eq!(with_effort, r#"{"model": "gpt-x", "instructions": "sys", "input": "in", "max_output_tokens": 4096, "reasoning": {"effort": "high"}}"#);

        let without_effort = compact_body_with_reasoning("gpt-x", "sys", "in", 4096, None);
        assert_eq!(without_effort, r#"{"model": "gpt-x", "instructions": "sys", "input": "in", "max_output_tokens": 4096}"#);

        let empty_effort = compact_body_with_reasoning("gpt-x", "sys", "in", 4096, Some(""));
        assert_eq!(empty_effort, without_effort);
    }

    #[test]
    fn compact_body_escapes_non_ascii() {
        let body = compact_body_no_reasoning("m", "café", "in", 1);
        assert!(body.contains("caf\\u00e9"));
    }
}
