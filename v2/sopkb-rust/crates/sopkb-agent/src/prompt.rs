//! `SYSTEM_PROMPT` and `build_agent_messages` (`agent_chat.py` lines 12-19, 108-131).
//! docs/port/port-mapping-d-agent-mcp.md §5.1, §5.4.

use sopkb_llm::Message;

/// Byte-exact copy of the Python triple-quoted literal, including the trailing `\n`
/// after the last line. P-A28: PRESERVE -- a non-blank `chat_prompt` profile override
/// fully replaces this (never merged), so an override that omits the `allow_proposed`
/// caveat sentence silently removes that caveat, by design.
pub const SYSTEM_PROMPT: &str = "You are an SOP Knowledge Bundle agent.\nUse the supplied agent.context JSON as the authority.\nUse decision_rules for conditional task handling.\nDo not infer conditions from prose when structured rules exist.\nIf retrieval.mode is scenario-concepts, use the detected concepts and linked OKF context as the retrieval basis.\nProposed knowledge may be used only when allow_proposed is true, and must be caveated as draft/non-final.\nReturn concise Markdown with: Decision, Rule Evaluations, Evidence, Caveats.\n";

/// The three concatenated Python string literals that open the user message, verbatim
/// including the single spaces at the joins and the trailing blank line before the
/// JSON payload.
const USER_PREAMBLE: &str = "Evaluate the scenario against the SOP task context. If a decision rule condition is false and the rule has otherwise.action_required=false, treat the obligation as not required by that rule.\n\n";

/// `build_agent_messages(context, scenario, allow_proposed, profile_id, bundle_override) -> [system, user]`.
/// `context` is the ENTIRE `scenario_agent_context` result, nested under `agent_context`
/// in the outbound payload -- not trimmed for token budget (P-A30, DECIDE lean PRESERVE).
///
/// `bundle_override`, when non-blank, wins over even a non-blank per-profile
/// `chat_prompt` -- see `sopkb_core::prompt_overrides`'s own doc comment for the
/// full precedence rationale (mirrors `sopkb_mining::build_author_messages`).
pub fn build_agent_messages(
    context: &serde_json::Value,
    scenario: &str,
    allow_proposed: bool,
    profile_id: Option<&str>,
    bundle_override: Option<&str>,
) -> Vec<Message> {
    let payload = serde_json::json!({
        "scenario": scenario,
        "allow_proposed": allow_proposed,
        "agent_context": context,
    });

    let system_prompt = match bundle_override.map(str::trim).filter(|s| !s.is_empty()) {
        Some(over) => over.to_string(),
        None => {
            // P-A28: no env override for chat_prompt; `resolve` already trims. When
            // `profile_id` is None this still consults the DEFAULT profile's
            // chat_prompt -- "no profile selected" is not "no override".
            let resolved = sopkb_config::resolve("chat_prompt", profile_id);
            if resolved.is_empty() { SYSTEM_PROMPT.to_string() } else { resolved }
        }
    };

    let user_content = format!("{USER_PREAMBLE}{}", sopkb_fmt::to_canonical_json(&payload));

    vec![Message::system(system_prompt), Message::user(user_content)]
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn system_prompt_is_byte_exact() {
        let expected = "You are an SOP Knowledge Bundle agent.\n\
Use the supplied agent.context JSON as the authority.\n\
Use decision_rules for conditional task handling.\n\
Do not infer conditions from prose when structured rules exist.\n\
If retrieval.mode is scenario-concepts, use the detected concepts and linked OKF context as the retrieval basis.\n\
Proposed knowledge may be used only when allow_proposed is true, and must be caveated as draft/non-final.\n\
Return concise Markdown with: Decision, Rule Evaluations, Evidence, Caveats.\n";
        assert_eq!(SYSTEM_PROMPT, expected);
        assert!(SYSTEM_PROMPT.starts_with("You"));
        assert!(SYSTEM_PROMPT.ends_with("Caveats.\n"));
        assert!(!SYSTEM_PROMPT.ends_with("Caveats.\n\n"));
    }

    #[test]
    fn user_preamble_is_byte_exact() {
        let expected = "Evaluate the scenario against the SOP task context. If a decision rule condition is false and the rule has otherwise.action_required=false, treat the obligation as not required by that rule.\n\n";
        assert_eq!(USER_PREAMBLE, expected);
    }

    #[test]
    #[serial]
    fn no_profile_falls_back_to_system_prompt() {
        with_settings_path(|| {
            // Empty settings store, no such profile -> resolve("chat_prompt", ...) returns "".
            let context = serde_json::json!({});
            let messages = build_agent_messages(&context, "scenario", false, Some("nonexistent-profile-xyz"), None);
            assert_eq!(messages[0].role, "system");
            assert_eq!(messages[0].content, SYSTEM_PROMPT);
        });
    }

    #[test]
    #[serial]
    fn whitespace_only_chat_prompt_falls_back_to_system_prompt() {
        with_settings_path(|| {
            let profile = sopkb_config::ModelProfile { id: "p1".into(), chat_prompt: "   \n\t  ".into(), ..Default::default() };
            sopkb_config::save_profile(&profile).unwrap();
            let messages = build_agent_messages(&serde_json::json!({}), "scenario", false, Some("p1"), None);
            assert_eq!(messages[0].content, SYSTEM_PROMPT);
        });
    }

    #[test]
    #[serial]
    fn non_blank_chat_prompt_fully_replaces_system_prompt() {
        with_settings_path(|| {
            let profile = sopkb_config::ModelProfile { id: "p1".into(), chat_prompt: "Custom prompt, no caveat sentence.".into(), ..Default::default() };
            sopkb_config::save_profile(&profile).unwrap();
            let messages = build_agent_messages(&serde_json::json!({}), "scenario", false, Some("p1"), None);
            assert_eq!(messages[0].content, "Custom prompt, no caveat sentence.");
            assert_ne!(messages[0].content, SYSTEM_PROMPT);
        });
    }

    #[test]
    #[serial]
    fn null_profile_id_still_uses_default_profiles_chat_prompt() {
        with_settings_path(|| {
            let profile = sopkb_config::ModelProfile { id: "p1".into(), chat_prompt: "Default profile override.".into(), ..Default::default() };
            sopkb_config::save_profile(&profile).unwrap();
            sopkb_config::set_default_profile("p1").unwrap();
            // profile_id: None is NOT "no override" -- it means "the default profile".
            let messages = build_agent_messages(&serde_json::json!({}), "scenario", false, None, None);
            assert_eq!(messages[0].content, "Default profile override.");
        });
    }

    #[test]
    #[serial]
    fn user_message_contains_preamble_then_canonical_payload() {
        with_settings_path(|| {
            let context = serde_json::json!({"task": {"title": "T"}});
            let messages = build_agent_messages(&context, "my scenario", true, Some("nonexistent-profile-xyz"), None);
            assert_eq!(messages[1].role, "user");
            let payload = serde_json::json!({"scenario": "my scenario", "allow_proposed": true, "agent_context": context});
            let expected = format!("{USER_PREAMBLE}{}", sopkb_fmt::to_canonical_json(&payload));
            assert_eq!(messages[1].content, expected);
        });
    }

    #[test]
    #[serial]
    fn bundle_override_wins_over_a_non_blank_profile_chat_prompt() {
        with_settings_path(|| {
            let profile = sopkb_config::ModelProfile { id: "p1".into(), chat_prompt: "Profile-level prompt.".into(), ..Default::default() };
            sopkb_config::save_profile(&profile).unwrap();
            let messages = build_agent_messages(&serde_json::json!({}), "scenario", false, Some("p1"), Some("Bundle-level prompt."));
            assert_eq!(messages[0].content, "Bundle-level prompt.");
        });
    }

    #[test]
    #[serial]
    fn blank_bundle_override_falls_back_to_profile_chat_prompt() {
        with_settings_path(|| {
            let profile = sopkb_config::ModelProfile { id: "p1".into(), chat_prompt: "Profile-level prompt.".into(), ..Default::default() };
            sopkb_config::save_profile(&profile).unwrap();
            let messages = build_agent_messages(&serde_json::json!({}), "scenario", false, Some("p1"), Some("   \n  "));
            assert_eq!(messages[0].content, "Profile-level prompt.");
        });
    }
}
