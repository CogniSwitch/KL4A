//! §4.5 Review. Split one command per action (`approve_item`/`reject_item`/
//! `defer_item`/`comment_item`/`edit_item`) so the frontend cannot construct an
//! invalid action/field combination.
//!
//! Not double-locked: `sopkb_review::{approve_item,reject_item,defer_item,
//! comment_item,edit_item}` already wrap their whole read-modify-write-sync
//! sequence in `sopkb_review::lock::with_bundle_lock` internally (confirmed by
//! reading `sopkb-review/src/review.rs` -- every action calls
//! `with_bundle_lock(bundle_dir, || { ... })` around its own body). Wrapping them
//! again in `WorkbenchHandle::begin_mutation()` would be a second, independent
//! locking mechanism guarding the same critical section for no benefit -- these
//! commands intentionally do NOT take that guard.

use serde::Deserialize;
use serde_json::Value;
use tauri::{AppHandle, State};

use crate::error::{AppError, CmdResult};
use crate::events::{emit_bundle_state_changed, REVIEW_SCOPE};
use crate::state::{resolve_bundle_dir, AppState};

/// §4.5's `reviewer` resolution, done here because `sopkb_review`'s functions take
/// `reviewer: &str` directly and know nothing about settings: an explicit non-blank
/// `reviewer` param wins, else `sopkb_config::reviewer_name()` (itself infallible,
/// `""` if unset), else the literal `"local:user"`.
pub(crate) fn resolve_reviewer(reviewer: Option<&str>) -> String {
    if let Some(r) = reviewer {
        let trimmed = r.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let configured = sopkb_config::reviewer_name();
    if !configured.trim().is_empty() {
        return configured;
    }
    "local:user".to_string()
}

/// Every review action ends with a full `sopkb_export::sync_okf_bundle` resync (see
/// `sopkb_review::review`'s own `with_bundle_lock` bodies) -- on a bundle with many
/// sections/items this is real, non-trivial disk I/O, not a quick JSON tweak. Left as
/// a plain synchronous `#[tauri::command]`, this blocks whatever thread Tauri
/// schedules the command handler on for the full resync duration -- a real,
/// reported bug ("clicking Comment hangs the UI for a bit"), not merely a slow
/// response the frontend's own busy-state spinner (`ActionButton`'s `busy`/disabled
/// prop, already correct) was failing to show. `async fn` + `spawn_blocking` moves
/// that blocking work onto Tokio's dedicated blocking-thread pool instead, exactly
/// the same fix already applied to `run_ingest_pipeline`/`run_agent_chat` for the
/// identical reason.
macro_rules! review_action_command {
    ($name:ident, $backend:path) => {
        #[tauri::command(rename_all = "snake_case")]
        pub async fn $name(
            app: AppHandle,
            state: State<'_, AppState>,
            item_id: String,
            rationale: String,
            reviewer: Option<String>,
            key: Option<String>,
        ) -> CmdResult<Value> {
            let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
            let resolved_reviewer = resolve_reviewer(reviewer.as_deref());
            // Computed BEFORE the move below -- `bundle_dir` itself is consumed by
            // the spawned closure, but the event we emit afterward still needs it.
            let bundle_key = bundle_dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            let bundle_dir_for_task = bundle_dir.clone();
            let event = tauri::async_runtime::spawn_blocking(move || $backend(&bundle_dir_for_task, &item_id, &resolved_reviewer, &rationale))
                .await
                .map_err(|err| AppError::new("Io", format!("{} task did not complete: {err}", stringify!($name))))?
                .map_err(AppError::from)?;
            emit_bundle_state_changed(&app, &bundle_key, REVIEW_SCOPE);
            Ok(event)
        }
    };
}

review_action_command!(approve_item, sopkb_review::approve_item);
review_action_command!(reject_item, sopkb_review::reject_item);
review_action_command!(defer_item, sopkb_review::defer_item);
review_action_command!(comment_item, sopkb_review::comment_item);

/// §4.5's tagged value union at the command boundary (PORT_PLAN's own instruction:
/// "adopt the tagged value union... at the command boundary"). `sopkb_review::edit_item`
/// itself has no such type -- it takes a plain `value: &str` for every field and
/// special-cases `confidence` internally via Python-float-grammar parsing -- so this
/// enum exists purely to give the frontend a structured argument instead of a bare
/// string, and is converted to that same plain string right before the one call in.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum EditValue {
    Text(String),
    Confidence(f64),
}

impl EditValue {
    fn as_backend_string(&self) -> String {
        match self {
            EditValue::Text(s) => s.clone(),
            // f64::to_string() never uses scientific notation for values in the
            // valid confidence range (0.0..=1.0), so this always parses back
            // cleanly through `sopkb_review`'s Python-float-grammar parser.
            EditValue::Confidence(v) => v.to_string(),
        }
    }
}

/// Same blocking-resync reasoning as `review_action_command!` above applies here --
/// `edit_item` isn't macro-generated (its extra `field`/`value` params don't fit that
/// shape), but it ends in the identical `with_bundle_lock` + full OKF resync.
#[tauri::command(rename_all = "snake_case")]
pub async fn edit_item(
    app: AppHandle,
    state: State<'_, AppState>,
    item_id: String,
    field: String,
    value: EditValue,
    rationale: String,
    reviewer: Option<String>,
    key: Option<String>,
) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let resolved_reviewer = resolve_reviewer(reviewer.as_deref());
    let value_str = value.as_backend_string();
    let bundle_key = bundle_dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    let bundle_dir_for_task = bundle_dir.clone();
    let event = tauri::async_runtime::spawn_blocking(move || sopkb_review::edit_item(&bundle_dir_for_task, &item_id, &field, &value_str, &resolved_reviewer, &rationale))
        .await
        .map_err(|err| AppError::new("Io", format!("edit_item task did not complete: {err}")))?
        .map_err(AppError::from)?;
    emit_bundle_state_changed(&app, &bundle_key, REVIEW_SCOPE);
    Ok(event)
}

/// Reads `.sopkb/reviews.json` directly (§4.5: `list_review_events`), optionally
/// filtered to one knowledge item. Pure once the raw JSON is in hand, so factored
/// out for a unit test independent of the state-resolution/IO plumbing above.
pub(crate) fn filter_review_events(events: Value, item_id: Option<&str>) -> Vec<Value> {
    let all: Vec<Value> = events.as_array().cloned().unwrap_or_default();
    match item_id {
        Some(id) if !id.is_empty() => {
            all.into_iter().filter(|e| e.get("knowledge_item_id").and_then(|v| v.as_str()) == Some(id)).collect()
        }
        _ => all,
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_review_events(state: State<AppState>, item_id: Option<String>, key: Option<String>) -> CmdResult<Vec<Value>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let events = sopkb_core::store::read_state_json(&bundle_dir, "reviews.json", serde_json::json!([])).map_err(AppError::from)?;
    Ok(filter_review_events(events, item_id.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn resolve_reviewer_prefers_explicit_non_blank_argument() {
        assert_eq!(resolve_reviewer(Some("alice")), "alice");
        assert_eq!(resolve_reviewer(Some("  alice  ")), "alice");
    }

    #[test]
    #[serial(sopkb_settings_path_env)]
    fn resolve_reviewer_falls_back_to_local_user_when_nothing_configured() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("nonexistent-settings.json");
        unsafe {
            std::env::set_var("SOPKB_SETTINGS_PATH", &settings_path);
        }
        assert_eq!(resolve_reviewer(None), "local:user");
        assert_eq!(resolve_reviewer(Some("   ")), "local:user");
        unsafe {
            std::env::remove_var("SOPKB_SETTINGS_PATH");
        }
    }

    #[test]
    #[serial(sopkb_settings_path_env)]
    fn resolve_reviewer_falls_back_to_configured_settings_reviewer_name() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        unsafe {
            std::env::set_var("SOPKB_SETTINGS_PATH", &settings_path);
        }
        sopkb_config::set_reviewer_name("configured-reviewer").unwrap();
        assert_eq!(resolve_reviewer(None), "configured-reviewer");
        unsafe {
            std::env::remove_var("SOPKB_SETTINGS_PATH");
        }
    }

    #[test]
    fn edit_value_text_passes_through_verbatim() {
        assert_eq!(EditValue::Text("hello".to_string()).as_backend_string(), "hello");
    }

    #[test]
    fn edit_value_confidence_stringifies_without_scientific_notation() {
        assert_eq!(EditValue::Confidence(0.95).as_backend_string(), "0.95");
        assert_eq!(EditValue::Confidence(0.0).as_backend_string(), "0");
        assert_eq!(EditValue::Confidence(1.0).as_backend_string(), "1");
    }

    #[test]
    fn edit_value_deserializes_tagged_shape() {
        let text: EditValue = serde_json::from_value(serde_json::json!({"type": "text", "value": "hi"})).unwrap();
        assert!(matches!(text, EditValue::Text(s) if s == "hi"));
        let conf: EditValue = serde_json::from_value(serde_json::json!({"type": "confidence", "value": 0.5})).unwrap();
        assert!(matches!(conf, EditValue::Confidence(v) if v == 0.5));
    }

    #[test]
    fn filter_review_events_no_filter_returns_everything() {
        let events = serde_json::json!([
            {"id": "review-000001", "knowledge_item_id": "ki-1"},
            {"id": "review-000002", "knowledge_item_id": "ki-2"},
        ]);
        assert_eq!(filter_review_events(events, None).len(), 2);
    }

    #[test]
    fn filter_review_events_filters_by_item_id() {
        let events = serde_json::json!([
            {"id": "review-000001", "knowledge_item_id": "ki-1"},
            {"id": "review-000002", "knowledge_item_id": "ki-2"},
        ]);
        let filtered = filter_review_events(events, Some("ki-2"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0]["id"], "review-000002");
    }

    #[test]
    fn filter_review_events_empty_item_id_string_returns_everything() {
        let events = serde_json::json!([{"id": "review-000001", "knowledge_item_id": "ki-1"}]);
        assert_eq!(filter_review_events(events, Some("")).len(), 1);
    }

    #[test]
    fn filter_review_events_non_array_input_is_empty() {
        assert!(filter_review_events(serde_json::json!(null), None).is_empty());
    }
}
