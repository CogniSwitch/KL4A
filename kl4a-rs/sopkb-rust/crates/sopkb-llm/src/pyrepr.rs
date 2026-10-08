//! Best-effort Python `repr()` of a JSON-sourced value, used only to format the
//! debug tail of the "Unexpected Azure OpenAI response(s) shape" errors
//! (`okf_author.response_text` / `agent_chat.response_text`, both raise
//! `RuntimeError("Unexpected Azure OpenAI response{,s} shape: " + repr(response_body))`).
//! This is a malformed-provider-response error path with no differential-test gate
//! (no fixture ever produces it); fidelity is approximate, not byte-exact.

use serde_json::Value;

pub(crate) fn python_repr(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(b) => {
            if *b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                n.to_string()
            } else if let Some(f) = n.as_f64() {
                sopkb_fmt::py_float(f)
            } else {
                n.to_string()
            }
        }
        Value::String(s) => python_repr_str(s),
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(python_repr).collect();
            format!("[{}]", inner.join(", "))
        }
        Value::Object(map) => {
            let inner: Vec<String> = map.iter().map(|(k, v)| format!("{}: {}", python_repr_str(k), python_repr(v))).collect();
            format!("{{{}}}", inner.join(", "))
        }
    }
}

/// Python prefers single quotes, switching to double quotes only when the string
/// contains a `'` and no `"`.
fn python_repr_str(s: &str) -> String {
    let use_double = s.contains('\'') && !s.contains('"');
    let quote = if use_double { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scalars_and_containers() {
        assert_eq!(python_repr(&Value::Null), "None");
        assert_eq!(python_repr(&json!(true)), "True");
        assert_eq!(python_repr(&json!(false)), "False");
        assert_eq!(python_repr(&json!(42)), "42");
        assert_eq!(python_repr(&json!("hi")), "'hi'");
        assert_eq!(python_repr(&json!([1, "a", null])), "[1, 'a', None]");
        assert_eq!(python_repr(&json!({"k": 1})), "{'k': 1}");
    }

    #[test]
    fn string_quoting_switches_on_embedded_apostrophe() {
        assert_eq!(python_repr(&json!("it's")), "\"it's\"");
        assert_eq!(python_repr(&json!("plain")), "'plain'");
    }
}
