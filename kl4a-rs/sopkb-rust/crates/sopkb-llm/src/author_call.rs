//! `okf_author.call_azure_responses` -- no `reasoning` key, no `status == "incomplete"`
//! guard in `response_text` (docs/port/port-mapping-b-mining-settings.md §3.2.4/§3.2's
//! note on the asymmetry with `agent_chat`'s equivalent; preserved as-is, not "fixed").

use crate::message::{flatten_messages, Message};
use crate::request::{azure_responses_url, compact_body_no_reasoning, parse_py_int, post_and_parse};
use crate::response::response_text_author;
use crate::transport::{Transport, UreqTransport};
use sopkb_core::error::Result;

pub fn author_call(messages: &[Message], profile_id: Option<&str>) -> Result<String> {
    author_call_with_transport(messages, profile_id, &UreqTransport)
}

/// Same evaluation order as the Python original (matters for which config error
/// surfaces first when several fields are missing at once): model, then
/// max_output_tokens, then api_key, then auth_style, then base_url (via the URL
/// builder), then timeout_seconds (resolved last, right before the POST).
pub fn author_call_with_transport(messages: &[Message], profile_id: Option<&str>, transport: &dyn Transport) -> Result<String> {
    let model = sopkb_config::require("model", "a model/deployment name", profile_id)?;
    let max_output_tokens = parse_py_int(&sopkb_config::resolve("max_output_tokens", profile_id))?;
    let api_key = sopkb_config::require("api_key", "an LLM API key", profile_id)?;
    let auth_style = sopkb_config::resolve("auth_style", profile_id);
    let url = azure_responses_url(profile_id)?;

    let (system_content, user_content) = flatten_messages(messages);
    let body = compact_body_no_reasoning(&model, &system_content, &user_content, max_output_tokens);

    let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
    headers.extend(sopkb_config::auth_headers(&auth_style, &api_key));

    let timeout_secs = parse_py_int(&sopkb_config::resolve("timeout_seconds", profile_id))?.max(0) as u64;

    let response_json = post_and_parse(transport, url, headers, body.into_bytes(), timeout_secs, "author")?;
    response_text_author(&response_json)
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

    fn save_full_profile() {
        let profile = sopkb_config::ModelProfile {
            id: "p1".into(),
            name: "One".into(),
            base_url: "https://example.test///".into(),
            api_key: "secret-key".into(),
            model: "gpt-x".into(),
            ..Default::default()
        };
        sopkb_config::save_profile(&profile).unwrap();
    }

    #[test]
    #[serial]
    fn happy_path_builds_expected_request_and_extracts_text() {
        with_settings_path(|| {
            save_full_profile();
            let transport = MockTransport::ok(200, br#"{"output_text": "hello from model"}"#.to_vec());
            let messages = vec![Message::system("sys prompt"), Message::user("user prompt")];
            let result = author_call_with_transport(&messages, None, &transport).unwrap();
            assert_eq!(result, "hello from model");

            let req = transport.last_request().unwrap();
            assert_eq!(req.url, "https://example.test/responses");
            assert_eq!(req.timeout_secs, 60);
            assert!(req.headers.contains(&("Content-Type".to_string(), "application/json".to_string())));
            assert!(req.headers.contains(&("api-key".to_string(), "secret-key".to_string())));
            let body_str = String::from_utf8(req.body).unwrap();
            assert_eq!(
                body_str,
                r#"{"model": "gpt-x", "instructions": "sys prompt", "input": "user prompt", "max_output_tokens": 4096}"#
            );
        });
    }

    #[test]
    #[serial]
    fn openai_auth_style_sends_bearer_header() {
        with_settings_path(|| {
            let profile = sopkb_config::ModelProfile {
                id: "p1".into(),
                base_url: "https://example.test".into(),
                api_key: "k".into(),
                model: "m".into(),
                auth_style: "openai".into(),
                ..Default::default()
            };
            sopkb_config::save_profile(&profile).unwrap();
            let transport = MockTransport::ok(200, br#"{"output_text": "ok"}"#.to_vec());
            author_call_with_transport(&[Message::user("hi")], None, &transport).unwrap();
            let req = transport.last_request().unwrap();
            assert!(req.headers.contains(&("Authorization".to_string(), "Bearer k".to_string())));
        });
    }

    #[test]
    #[serial]
    fn http_error_status_wraps_with_author_label() {
        with_settings_path(|| {
            save_full_profile();
            let transport = MockTransport::ok(500, b"boom".to_vec());
            let err = author_call_with_transport(&[Message::user("hi")], None, &transport).unwrap_err().to_string();
            assert_eq!(err, "Azure OpenAI author request failed: HTTP 500: boom");
        });
    }

    #[test]
    #[serial]
    fn missing_model_fails_before_missing_api_key() {
        with_settings_path(|| {
            // No profile saved at all: model is checked first per the evaluation order.
            let transport = MockTransport::ok(200, b"{}".to_vec());
            let err = author_call_with_transport(&[Message::user("hi")], None, &transport).unwrap_err().to_string();
            assert_eq!(
                err,
                "Missing a model/deployment name. Configure a model profile on the Settings screen or AZURE_OPENAI_DEPLOYMENT / OPENAI_MODEL."
            );
            assert!(transport.last_request().is_none(), "must fail before ever reaching the network");
        });
    }

    #[test]
    #[serial]
    fn incomplete_status_now_errors_on_the_author_path() {
        // P-M13 closed 2026-08-22 -- see sopkb_llm::response's module doc.
        with_settings_path(|| {
            save_full_profile();
            let transport = MockTransport::ok(200, br#"{"status": "incomplete", "output_text": "still used"}"#.to_vec());
            let err = author_call_with_transport(&[Message::user("hi")], None, &transport).unwrap_err().to_string();
            assert!(err.starts_with("Azure OpenAI response incomplete"), "unexpected error: {err}");
        });
    }
}
