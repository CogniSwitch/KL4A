//! `agent_chat.call_azure_responses` -- adds a `reasoning` key when
//! `reasoning_effort` is set, and `response_text` checks `status == "incomplete"`
//! before extracting any text (docs/port/port-mapping-d-agent-mcp.md §5.5/§5.6).

use crate::message::{flatten_messages, Message};
use crate::request::{azure_responses_url, compact_body_with_reasoning, parse_py_int, post_and_parse};
use crate::response::response_text_chat;
use crate::transport::{Transport, UreqTransport};
use sopkb_core::error::Result;

pub fn chat_call(messages: &[Message], profile_id: Option<&str>) -> Result<String> {
    chat_call_with_transport(messages, profile_id, &UreqTransport)
}

/// Evaluation order: model, then max_output_tokens, then reasoning_effort (resolve
/// only, never fails), then api_key, then auth_style, then base_url, then
/// timeout_seconds (resolved last, right before the POST).
pub fn chat_call_with_transport(messages: &[Message], profile_id: Option<&str>, transport: &dyn Transport) -> Result<String> {
    let model = sopkb_config::require("model", "a model/deployment name", profile_id)?;
    let max_output_tokens = parse_py_int(&sopkb_config::resolve("max_output_tokens", profile_id))?;
    let reasoning_effort = sopkb_config::resolve("reasoning_effort", profile_id);
    let api_key = sopkb_config::require("api_key", "an LLM API key", profile_id)?;
    let auth_style = sopkb_config::resolve("auth_style", profile_id);
    let url = azure_responses_url(profile_id)?;

    let (system_content, user_content) = flatten_messages(messages);
    let effort = if reasoning_effort.is_empty() { None } else { Some(reasoning_effort.as_str()) };
    let body = compact_body_with_reasoning(&model, &system_content, &user_content, max_output_tokens, effort);

    let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
    headers.extend(sopkb_config::auth_headers(&auth_style, &api_key));

    let timeout_secs = parse_py_int(&sopkb_config::resolve("timeout_seconds", profile_id))?.max(0) as u64;

    let response_json = post_and_parse(transport, url, headers, body.into_bytes(), timeout_secs, "agent")?;
    response_text_chat(&response_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MockTransport;
    use serial_test::serial;
    use tempfile::tempdir;

    fn with_settings_path<F: FnOnce()>(f: F) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        unsafe { std::env::set_var("SOPKB_SETTINGS_PATH", &path) };
        f();
        unsafe { std::env::remove_var("SOPKB_SETTINGS_PATH") };
    }

    #[test]
    #[serial]
    fn reasoning_key_present_only_when_effort_set() {
        with_settings_path(|| {
            let profile = sopkb_config::ModelProfile {
                id: "p1".into(),
                base_url: "https://example.test".into(),
                api_key: "k".into(),
                model: "m".into(),
                reasoning_effort: "high".into(),
                ..Default::default()
            };
            sopkb_config::save_profile(&profile).unwrap();
            let transport = MockTransport::ok(200, br#"{"output_text": "ok"}"#.to_vec());
            chat_call_with_transport(&[Message::user("hi")], None, &transport).unwrap();
            let body = String::from_utf8(transport.last_request().unwrap().body).unwrap();
            assert!(body.contains(r#""reasoning": {"effort": "high"}"#));
        });
    }

    #[test]
    #[serial]
    fn reasoning_key_absent_when_effort_unset() {
        with_settings_path(|| {
            let profile =
                sopkb_config::ModelProfile { id: "p1".into(), base_url: "https://example.test".into(), api_key: "k".into(), model: "m".into(), ..Default::default() };
            sopkb_config::save_profile(&profile).unwrap();
            let transport = MockTransport::ok(200, br#"{"output_text": "ok"}"#.to_vec());
            chat_call_with_transport(&[Message::user("hi")], None, &transport).unwrap();
            let body = String::from_utf8(transport.last_request().unwrap().body).unwrap();
            assert!(!body.contains("reasoning"));
        });
    }

    #[test]
    #[serial]
    fn incomplete_status_raises_before_extraction() {
        with_settings_path(|| {
            let profile =
                sopkb_config::ModelProfile { id: "p1".into(), base_url: "https://example.test".into(), api_key: "k".into(), model: "m".into(), ..Default::default() };
            sopkb_config::save_profile(&profile).unwrap();
            let transport = MockTransport::ok(200, br#"{"status": "incomplete", "output_text": "ignored"}"#.to_vec());
            let err = chat_call_with_transport(&[Message::user("hi")], None, &transport).unwrap_err().to_string();
            assert!(err.starts_with("Azure OpenAI response incomplete"));
        });
    }

    #[test]
    #[serial]
    fn http_error_status_wraps_with_agent_label() {
        with_settings_path(|| {
            let profile =
                sopkb_config::ModelProfile { id: "p1".into(), base_url: "https://example.test".into(), api_key: "k".into(), model: "m".into(), ..Default::default() };
            sopkb_config::save_profile(&profile).unwrap();
            let transport = MockTransport::ok(401, b"unauthorized".to_vec());
            let err = chat_call_with_transport(&[Message::user("hi")], None, &transport).unwrap_err().to_string();
            assert_eq!(err, "Azure OpenAI agent request failed: HTTP 401: unauthorized");
        });
    }

    #[test]
    #[serial]
    fn connection_failure_propagates_unwrapped() {
        with_settings_path(|| {
            let profile =
                sopkb_config::ModelProfile { id: "p1".into(), base_url: "https://example.test".into(), api_key: "k".into(), model: "m".into(), ..Default::default() };
            sopkb_config::save_profile(&profile).unwrap();
            let transport = MockTransport::err("dns resolution failed");
            let err = chat_call_with_transport(&[Message::user("hi")], None, &transport).unwrap_err().to_string();
            assert_eq!(err, "dns resolution failed");
        });
    }
}
