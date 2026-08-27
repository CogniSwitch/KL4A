//! Transcript read/append/cap -- the caller-side persistence tail of Python's
//! `web_app.handle_agent_form` (`agent_chat.json`, lines 494-500 of `web_app.py`).
//! docs/port/port-mapping-d-agent-mcp.md §5.7.
//!
//! P-A35: PRESERVE the single-shot model -- the transcript is never fed back into the
//! model as history; every `run_agent_chat` call is a fresh single-shot evaluation.
//! This module does not change that.
//!
//! DEVIATION (see DEVIATIONS.md): Python caps the transcript at a hard-coded last-25
//! entries on every write, destroying older entries with no way to recover them or
//! configure the limit. PORT_PLAN.md P-A35 explicitly flags this as hard to defend
//! and recommends raising or parameterizing it. This module keeps the DEFAULT at 25
//! (so the reference-fixture byte-identical test at the default cap is unaffected)
//! but exposes the cap as a parameter (`append_agent_chat_entry_capped`) so a future
//! caller (Settings screen, Phase 9 Tauri command, etc.) can choose a larger or
//! unlimited value instead of silently losing chat history.

use serde_json::Value;
use sopkb_core::error::Result;
use sopkb_core::store;
use std::path::Path;

const STATE_FILENAME: &str = "agent_chat.json";

/// Matches Python's hard-coded `transcript[-25:]`. Preserved as the default so the
/// reference-fixture differential test (fixed cap, byte-identical output) is
/// unaffected by this crate's added configurability.
pub const DEFAULT_TRANSCRIPT_CAP: usize = 25;

/// `read_agent_transcript(bundle_dir)`: canonical file wins over the legacy
/// `reports/agent_chat.json` alias (handled generically by `read_state_json`); a
/// non-list value at that path is treated as absent, matching Python's
/// `data if isinstance(data, list) else []`.
pub fn read_agent_transcript(bundle_dir: &Path) -> Result<Vec<Value>> {
    let data = store::read_state_json(bundle_dir, STATE_FILENAME, Value::Array(Vec::new()))?;
    Ok(match data {
        Value::Array(items) => items,
        _ => Vec::new(),
    })
}

/// `{created_at: utc_now()} merged with response` -- `created_at` conceptually first,
/// though key order is moot here since this workspace's `serde_json::Value` is
/// BTreeMap-backed (always emitted in sorted order by `to_canonical_json` anyway).
fn build_entry(response: Value) -> Value {
    let mut entry = serde_json::Map::new();
    entry.insert("created_at".to_string(), Value::String(store::utc_now()));
    if let Value::Object(fields) = response {
        for (k, v) in fields {
            entry.insert(k, v);
        }
    }
    Value::Object(entry)
}

/// Appends `response` (wrapped with a fresh `created_at`) to the bundle's transcript,
/// keeps only the last `cap` entries (oldest-first order preserved within that
/// window), writes the canonical `.sopkb/agent_chat.json`, and returns the entry that
/// was written. Pass `DEFAULT_TRANSCRIPT_CAP` for exact Python parity.
pub fn append_agent_chat_entry_capped(bundle_dir: &Path, response: Value, cap: usize) -> Result<Value> {
    let entry = build_entry(response);

    let mut transcript = read_agent_transcript(bundle_dir)?;
    transcript.push(entry.clone());
    let start = transcript.len().saturating_sub(cap);
    let capped = Value::Array(transcript.split_off(start));

    store::write_json(&store::state_path(bundle_dir, STATE_FILENAME), &capped)?;
    Ok(entry)
}

/// Exact Python parity: caps at the last 25 entries on every write.
pub fn append_agent_chat_entry(bundle_dir: &Path, response: Value) -> Result<Value> {
    append_agent_chat_entry_capped(bundle_dir, response, DEFAULT_TRANSCRIPT_CAP)
}

/// Removes every transcript entry whose `chat_id` matches (added by `chat::run_chat_turn`
/// -- see that module's own doc comment; entries written before this feature existed have
/// no `chat_id` field at all and never match). Returns the number of entries removed, so a
/// caller can tell "deleted a chat with 3 turns" from "there was nothing to delete" without
/// a second read. Rewrites the whole file rather than a targeted patch -- the transcript is
/// small enough (capped, see `DEFAULT_TRANSCRIPT_CAP`/`chat::CHAT_TRANSCRIPT_CAP`) that this
/// is simpler and no slower in practice than anything more surgical.
pub fn delete_chat(bundle_dir: &Path, chat_id: &str) -> Result<usize> {
    let transcript = read_agent_transcript(bundle_dir)?;
    let before = transcript.len();
    let kept: Vec<Value> = transcript.into_iter().filter(|e| e.get("chat_id").and_then(|v| v.as_str()) != Some(chat_id)).collect();
    let removed = before - kept.len();
    if removed > 0 {
        store::write_json(&store::state_path(bundle_dir, STATE_FILENAME), &Value::Array(kept))?;
    }
    Ok(removed)
}

/// Combines `run_agent_chat` with the transcript persistence tail -- the part of
/// `handle_agent_form` that is not raw-HTTP-form parsing (task_id/scenario/provider
/// defaulting and the `allow_proposed == "on"` checkbox semantics belong to the web
/// layer that calls this crate, not here). Uses the exact-parity 25-entry cap.
pub fn handle_agent_chat(
    bundle_dir: &Path,
    task_id: &str,
    scenario: &str,
    allow_proposed: bool,
    provider: &str,
    profile_id: Option<&str>,
) -> Result<Value> {
    let response = crate::run::run_agent_chat(bundle_dir, task_id, scenario, allow_proposed, provider, profile_id)?;
    append_agent_chat_entry(bundle_dir, response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sopkb_core::store as core_store;
    use tempfile::tempdir;

    fn new_bundle() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        core_store::create_bundle(&dir.path().join("b"), None).unwrap();
        dir
    }

    #[test]
    fn read_agent_transcript_defaults_to_empty() {
        let dir = new_bundle();
        let transcript = read_agent_transcript(&dir.path().join("b")).unwrap();
        assert!(transcript.is_empty());
    }

    #[test]
    fn read_agent_transcript_treats_non_list_as_empty() {
        let dir = new_bundle();
        let bundle_dir = dir.path().join("b");
        core_store::write_state_json(&bundle_dir, "agent_chat.json", &serde_json::json!({"not": "a list"})).unwrap();
        let transcript = read_agent_transcript(&bundle_dir).unwrap();
        assert!(transcript.is_empty());
    }

    #[test]
    fn append_creates_entry_with_created_at() {
        let dir = new_bundle();
        let bundle_dir = dir.path().join("b");
        let entry = append_agent_chat_entry(&bundle_dir, serde_json::json!({"provider": "context"})).unwrap();
        assert!(entry["created_at"].as_str().unwrap().ends_with('Z'));
        assert_eq!(entry["provider"], serde_json::json!("context"));

        let on_disk = read_agent_transcript(&bundle_dir).unwrap();
        assert_eq!(on_disk.len(), 1);
        assert_eq!(on_disk[0], entry);
    }

    #[test]
    fn default_cap_keeps_last_25_oldest_first() {
        let dir = new_bundle();
        let bundle_dir = dir.path().join("b");
        for i in 0..30 {
            append_agent_chat_entry(&bundle_dir, serde_json::json!({"seq": i})).unwrap();
        }
        let transcript = read_agent_transcript(&bundle_dir).unwrap();
        assert_eq!(transcript.len(), 25);
        // oldest-first order preserved within the surviving window: entries 5..=29 survive.
        assert_eq!(transcript[0]["seq"], serde_json::json!(5));
        assert_eq!(transcript[24]["seq"], serde_json::json!(29));
    }

    #[test]
    fn custom_cap_can_exceed_25() {
        let dir = new_bundle();
        let bundle_dir = dir.path().join("b");
        for i in 0..30 {
            append_agent_chat_entry_capped(&bundle_dir, serde_json::json!({"seq": i}), 100).unwrap();
        }
        let transcript = read_agent_transcript(&bundle_dir).unwrap();
        assert_eq!(transcript.len(), 30, "a raised cap must not destroy history the default 25-cap would have dropped");
        assert_eq!(transcript[0]["seq"], serde_json::json!(0));
        assert_eq!(transcript[29]["seq"], serde_json::json!(29));
    }

    #[test]
    fn delete_chat_removes_only_matching_entries_and_reports_the_count() {
        let dir = new_bundle();
        let bundle_dir = dir.path().join("b");
        append_agent_chat_entry(&bundle_dir, serde_json::json!({"chat_id": "c1", "seq": 0})).unwrap();
        append_agent_chat_entry(&bundle_dir, serde_json::json!({"chat_id": "c2", "seq": 1})).unwrap();
        append_agent_chat_entry(&bundle_dir, serde_json::json!({"chat_id": "c1", "seq": 2})).unwrap();

        let removed = delete_chat(&bundle_dir, "c1").unwrap();
        assert_eq!(removed, 2);

        let remaining = read_agent_transcript(&bundle_dir).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0]["chat_id"], serde_json::json!("c2"));
    }

    #[test]
    fn delete_chat_on_an_unknown_id_removes_nothing() {
        let dir = new_bundle();
        let bundle_dir = dir.path().join("b");
        append_agent_chat_entry(&bundle_dir, serde_json::json!({"chat_id": "c1", "seq": 0})).unwrap();

        let removed = delete_chat(&bundle_dir, "does-not-exist").unwrap();
        assert_eq!(removed, 0);
        assert_eq!(read_agent_transcript(&bundle_dir).unwrap().len(), 1);
    }

    #[test]
    fn delete_chat_never_matches_legacy_entries_with_no_chat_id() {
        let dir = new_bundle();
        let bundle_dir = dir.path().join("b");
        append_agent_chat_entry(&bundle_dir, serde_json::json!({"seq": 0})).unwrap();

        let removed = delete_chat(&bundle_dir, "c1").unwrap();
        assert_eq!(removed, 0);
        assert_eq!(read_agent_transcript(&bundle_dir).unwrap().len(), 1);
    }

    #[test]
    fn handle_agent_chat_persists_run_agent_chat_response() {
        let dir = new_bundle();
        let bundle_dir = dir.path().join("b");
        let items = serde_json::json!([
            {
                "id": "ki-1", "subject": "Staff", "predicate": "must confirm", "object": "patient identity",
                "source_id": "s", "section_id": "sec", "review_status": "accepted", "confidence": 0.9,
                "source_text": "Staff must confirm patient identity.", "span_status": "exact", "start_pos": 0, "end_pos": 10
            }
        ]);
        core_store::write_state_json(&bundle_dir, "items.json", &items).unwrap();

        let entry = handle_agent_chat(&bundle_dir, "auto", "confirm patient identity", false, "context", None).unwrap();
        assert_eq!(entry["provider"], serde_json::json!("context"));
        assert_eq!(entry["task_id"], serde_json::json!("auto"));
        assert!(entry["created_at"].is_string());

        let transcript = read_agent_transcript(&bundle_dir).unwrap();
        assert_eq!(transcript.len(), 1);
    }
}
