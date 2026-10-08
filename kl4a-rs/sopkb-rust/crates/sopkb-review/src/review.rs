//! `review.py`: the five review actions, the review-status state machine, and
//! `append_review_event`. docs/port/port-mapping-c-review-export.md §1;
//! docs/port/PORT_PLAN.md §6.7 (Phase 6). Every action is wrapped in
//! `crate::lock::with_bundle_lock` (P-W1, a new engineering requirement -- see lock.rs).

use crate::lock::with_bundle_lock;
use sopkb_core::error::{Result, SopkbError};
use sopkb_core::store;
use std::path::Path;

/// Event `action` values (§1.1). `commented` is an event action only, never an item
/// `review_status` (P-R13) -- `sopkb-review::validate`'s own `REVIEW_STATUSES` constant
/// is the separate, item-status enum.
pub const REVIEW_ACTIONS: &[&str] = &["approved", "rejected", "deferred", "commented", "edited"];
const TERMINAL_STATUSES: &[&str] = &["approved", "rejected"];
/// Fields `edit_item` may touch. Validated before any file access (P-R5).
pub const EDITABLE_FIELDS: &[&str] = &["subject", "predicate", "object", "source_text", "confidence"];

pub fn approve_item(bundle_dir: &Path, item_id: &str, reviewer: &str, rationale: &str) -> Result<serde_json::Value> {
    review_status_action(bundle_dir, item_id, "approved", reviewer, rationale)
}

pub fn reject_item(bundle_dir: &Path, item_id: &str, reviewer: &str, rationale: &str) -> Result<serde_json::Value> {
    review_status_action(bundle_dir, item_id, "rejected", reviewer, rationale)
}

pub fn defer_item(bundle_dir: &Path, item_id: &str, reviewer: &str, rationale: &str) -> Result<serde_json::Value> {
    review_status_action(bundle_dir, item_id, "deferred", reviewer, rationale)
}

/// Shared body for approve/reject/defer: load, check mutability, flip `review_status`,
/// rewrite `items.json` in full, append the event, then sync + refresh reports.
fn review_status_action(bundle_dir: &Path, item_id: &str, status: &str, reviewer: &str, rationale: &str) -> Result<serde_json::Value> {
    with_bundle_lock(bundle_dir, || {
        let mut items = load_items(bundle_dir)?;
        let idx = find_item_index(&items, item_id)?;
        ensure_mutable(&items[idx])?;
        let previous_status = items[idx].get("review_status").cloned().unwrap_or(serde_json::Value::Null);
        items[idx]["review_status"] = serde_json::Value::String(status.to_string());
        store::write_state_json(bundle_dir, "items.json", &serde_json::Value::Array(items))?;

        let event = append_review_event(
            bundle_dir,
            item_id,
            status,
            reviewer,
            rationale,
            Some(serde_json::json!({ "review_status": previous_status })),
            Some(serde_json::json!({ "review_status": status })),
        )?;
        sync_after_review(bundle_dir)?;
        Ok(event)
    })
}

/// The only action legal on a terminal item (P-R7): no `ensure_mutable` check, and
/// `items.json` is not touched at all -- the item itself is never modified.
pub fn comment_item(bundle_dir: &Path, item_id: &str, reviewer: &str, rationale: &str) -> Result<serde_json::Value> {
    with_bundle_lock(bundle_dir, || {
        let items = load_items(bundle_dir)?;
        find_item_index(&items, item_id)?; // existence check only; result discarded
        let event = append_review_event(bundle_dir, item_id, "commented", reviewer, rationale, None, None)?;
        sync_after_review(bundle_dir)?;
        Ok(event)
    })
}

/// Unconditionally forces `review_status = "edited"` (P-R8), overwriting
/// `proposed`/`deferred`/whatever it was, and does NOT record that status change in
/// the event -- only the edited field's before/after.
pub fn edit_item(bundle_dir: &Path, item_id: &str, field: &str, value: &str, reviewer: &str, rationale: &str) -> Result<serde_json::Value> {
    if !EDITABLE_FIELDS.contains(&field) {
        return Err(SopkbError::Value(format!("field is not editable: {field}")));
    }
    with_bundle_lock(bundle_dir, || {
        let mut items = load_items(bundle_dir)?;
        let idx = find_item_index(&items, item_id)?;
        ensure_mutable(&items[idx])?;

        let previous_value = items[idx].get(field).cloned().unwrap_or(serde_json::Value::Null);
        let new_value = parse_field_value(field, value)?;
        items[idx][field] = new_value.clone();
        items[idx]["review_status"] = serde_json::Value::String("edited".to_string());
        store::write_state_json(bundle_dir, "items.json", &serde_json::Value::Array(items))?;

        let event = append_review_event(
            bundle_dir,
            item_id,
            "edited",
            reviewer,
            rationale,
            Some(single_key_object(field, previous_value)),
            Some(single_key_object(field, new_value)),
        )?;
        sync_after_review(bundle_dir)?;
        Ok(event)
    })
}

fn single_key_object(key: &str, value: serde_json::Value) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    m.insert(key.to_string(), value);
    serde_json::Value::Object(m)
}

/// Appends one event to `.sopkb/reviews.json` and returns it. NOT independently
/// locked -- every current caller in this crate already holds the bundle lock for the
/// whole read-modify-write-sync sequence it's part of; do not call this directly from
/// outside an already-locked context without adding your own locking.
pub fn append_review_event(
    bundle_dir: &Path,
    item_id: &str,
    action: &str,
    reviewer: &str,
    rationale: &str,
    previous_value: Option<serde_json::Value>,
    new_value: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    if !REVIEW_ACTIONS.contains(&action) {
        return Err(SopkbError::Value(format!("invalid review action: {action}")));
    }
    let reviews_val = store::read_state_json(bundle_dir, "reviews.json", serde_json::json!([]))?;
    let mut reviews = reviews_val.as_array().cloned().unwrap_or_default();
    let event = serde_json::json!({
        "id": format!("review-{:06}", reviews.len() + 1),
        "knowledge_item_id": item_id,
        "action": action,
        "reviewer": reviewer,
        "timestamp": store::utc_now(),
        "rationale": rationale,
        "previous_value": previous_value.unwrap_or(serde_json::Value::Null),
        "new_value": new_value.unwrap_or(serde_json::Value::Null),
    });
    reviews.push(event.clone());
    store::write_state_json(bundle_dir, "reviews.json", &serde_json::Value::Array(reviews))?;
    Ok(event)
}

fn load_items(bundle_dir: &Path) -> Result<Vec<serde_json::Value>> {
    let raw = store::read_state_json(bundle_dir, "items.json", serde_json::Value::Null)?;
    match raw {
        serde_json::Value::Array(arr) => Ok(arr),
        _ => Err(SopkbError::NotFound("missing .sopkb/items.json".to_string())),
    }
}

fn find_item_index(items: &[serde_json::Value], item_id: &str) -> Result<usize> {
    items
        .iter()
        .position(|it| it.get("id").and_then(|v| v.as_str()) == Some(item_id))
        .ok_or_else(|| SopkbError::Value(format!("knowledge item not found: {item_id}")))
}

/// Tests membership in `TERMINAL_STATUSES` only (P-R1): an item with a missing,
/// null, or unrecognized `review_status` (e.g. `"foo"`) is mutable and every action
/// succeeds.
fn ensure_mutable(item: &serde_json::Value) -> Result<()> {
    if let Some(status) = item.get("review_status").and_then(|v| v.as_str()) {
        if TERMINAL_STATUSES.contains(&status) {
            return Err(SopkbError::Value(format!("cannot change terminal review status: {status}")));
        }
    }
    Ok(())
}

/// `confidence` is validated and stored as an f64; every other editable field is
/// stored verbatim as a string with no validation whatsoever (P-R4).
fn parse_field_value(field: &str, value: &str) -> Result<serde_json::Value> {
    if field != "confidence" {
        return Ok(serde_json::Value::String(value.to_string()));
    }
    let parsed = parse_python_float(value).map_err(|_| SopkbError::Value(format!("could not convert string to float: '{value}'")))?;
    if parsed.is_nan() {
        // P-R2, decided FIX: Python's NaN passes both bounds checks (`NaN < 0` and
        // `NaN > 1` are both false) and gets written as the bare `NaN` token -- invalid
        // per the JSON spec, only re-readable by Python's own permissive json.loads.
        // serde_json::Value has no representation for a non-finite JSON number, so
        // faithfully reproducing "accepted, stored as NaN" would require bypassing this
        // port's Value-based JSON pipeline for one field. Rejected instead, with the
        // same message as an out-of-bounds value (a NaN confidence is unusable either
        // way). See DEVIATIONS.md.
        return Err(SopkbError::Value("confidence must be between 0 and 1".to_string()));
    }
    if !(0.0..=1.0).contains(&parsed) {
        return Err(SopkbError::Value("confidence must be between 0 and 1".to_string()));
    }
    Ok(serde_json::json!(parsed))
}

/// Python `float()` grammar: strips surrounding whitespace, accepts `inf`/`infinity`/
/// `nan` case-insensitively with an optional sign, and (PEP 515) allows a single `_`
/// between two digits anywhere in the literal (`"1_0"` -> `10.0`).
fn parse_python_float(raw: &str) -> std::result::Result<f64, ()> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(());
    }
    let cleaned = strip_python_underscores(trimmed).ok_or(())?;
    cleaned.parse::<f64>().map_err(|_| ())
}

fn strip_python_underscores(s: &str) -> Option<String> {
    if !s.contains('_') {
        return Some(s.to_string());
    }
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
            let next_digit = i + 1 < chars.len() && chars[i + 1].is_ascii_digit();
            if !(prev_digit && next_digit) {
                return None;
            }
        }
    }
    Some(chars.into_iter().filter(|&c| c != '_').collect())
}

/// `export.sync_okf_bundle` (Python: a deferred import breaking the review<->export
/// import cycle; this port has no such cycle since `sopkb-review` depends on
/// `sopkb-export`, never the reverse -- §3.2). Additionally regenerates the four
/// standard reports (P-R15, decided FIX: Python's reports go stale after a review
/// action until the next full `validate_bundle` call; `write_standard_reports`
/// re-reads all its own state from disk, so it's a safe standalone call here too).
fn sync_after_review(bundle_dir: &Path) -> Result<()> {
    sopkb_export::sync_okf_bundle(bundle_dir)?;
    sopkb_export::reports::write_standard_reports(bundle_dir, &[], &[])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn bundle_with_one_item(status: &str) -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        store::create_bundle(&bundle_dir, None).unwrap();
        let items = serde_json::json!([{
            "id": "ki-a-000001", "item_type": "claim", "subject": "Staff", "predicate": "must",
            "object": "Staff must record contraindications.", "source_id": "a", "section_id": "a-sec-1",
            "source_text": "Staff must record contraindications.", "start_pos": 0, "end_pos": 10,
            "span_status": "exact", "derivation": "direct_text", "confidence": 0.82,
            "review_status": status, "metadata": {},
        }]);
        store::write_state_json(&bundle_dir, "items.json", &items).unwrap();
        dir
    }

    #[test]
    fn approve_then_reject_fails_terminal() {
        let dir = bundle_with_one_item("proposed");
        let bundle_dir = dir.path().join("bundle");
        let event = approve_item(&bundle_dir, "ki-a-000001", "local:user", "looks fine").unwrap();
        assert_eq!(event["action"], "approved");
        assert_eq!(event["id"], "review-000001");
        assert_eq!(event["previous_value"]["review_status"], "proposed");
        assert_eq!(event["new_value"]["review_status"], "approved");

        let err = reject_item(&bundle_dir, "ki-a-000001", "local:user", "too late").unwrap_err();
        assert_eq!(err.to_string(), "cannot change terminal review status: approved");

        let items = store::read_state_json(&bundle_dir, "items.json", serde_json::json!([])).unwrap();
        assert_eq!(items[0]["review_status"], "approved");
    }

    #[test]
    fn comment_allowed_on_terminal_item_and_does_not_touch_items_json() {
        let dir = bundle_with_one_item("approved");
        let bundle_dir = dir.path().join("bundle");
        let before = store::read_state_json(&bundle_dir, "items.json", serde_json::json!([])).unwrap();
        let event = comment_item(&bundle_dir, "ki-a-000001", "local:user", "confirmed with legal").unwrap();
        assert_eq!(event["action"], "commented");
        assert_eq!(event["previous_value"], serde_json::Value::Null);
        assert_eq!(event["new_value"], serde_json::Value::Null);
        let after = store::read_state_json(&bundle_dir, "items.json", serde_json::json!([])).unwrap();
        assert_eq!(before, after, "comment_item must never rewrite items.json");
    }

    #[test]
    fn defer_is_repeatable_and_always_appends_an_event() {
        let dir = bundle_with_one_item("deferred");
        let bundle_dir = dir.path().join("bundle");
        let e1 = defer_item(&bundle_dir, "ki-a-000001", "local:user", "still waiting").unwrap();
        let e2 = defer_item(&bundle_dir, "ki-a-000001", "local:user", "still waiting").unwrap();
        assert_eq!(e1["id"], "review-000001");
        assert_eq!(e2["id"], "review-000002");
        assert_eq!(e1["previous_value"], e2["previous_value"]);
    }

    #[test]
    fn edit_rejects_non_editable_field_before_touching_any_file() {
        let dir = bundle_with_one_item("proposed");
        let bundle_dir = dir.path().join("bundle");
        let err = edit_item(&bundle_dir, "ki-a-000001", "review_status", "approved", "local:user", "nope").unwrap_err();
        assert_eq!(err.to_string(), "field is not editable: review_status");
        assert!(!bundle_dir.join(".sopkb/reviews.json").exists(), "reviews.json must not be created");
    }

    #[test]
    fn edit_forces_edited_status_and_records_only_the_field() {
        let dir = bundle_with_one_item("proposed");
        let bundle_dir = dir.path().join("bundle");
        let event = edit_item(&bundle_dir, "ki-a-000001", "subject", "Escalation Staff", "local:user", "clarify").unwrap();
        assert_eq!(event["previous_value"], serde_json::json!({"subject": "Staff"}));
        assert_eq!(event["new_value"], serde_json::json!({"subject": "Escalation Staff"}));
        let items = store::read_state_json(&bundle_dir, "items.json", serde_json::json!([])).unwrap();
        assert_eq!(items[0]["review_status"], "edited");
        assert_eq!(items[0]["subject"], "Escalation Staff");
    }

    #[test]
    fn edit_confidence_accepts_valid_range_and_python_float_grammar() {
        let dir = bundle_with_one_item("proposed");
        let bundle_dir = dir.path().join("bundle");
        let event = edit_item(&bundle_dir, "ki-a-000001", "confidence", "  0.95  ", "local:user", "sure").unwrap();
        assert_eq!(event["new_value"], serde_json::json!({"confidence": 0.95}));

        let event2 = edit_item(&bundle_dir, "ki-a-000001", "confidence", "1_0e-2", "local:user", "underscore").unwrap();
        assert_eq!(event2["new_value"]["confidence"], 0.1);
    }

    #[test]
    fn edit_confidence_rejects_out_of_range_and_nan_and_garbage() {
        let dir = bundle_with_one_item("proposed");
        let bundle_dir = dir.path().join("bundle");
        assert_eq!(
            edit_item(&bundle_dir, "ki-a-000001", "confidence", "1.5", "u", "r").unwrap_err().to_string(),
            "confidence must be between 0 and 1"
        );
        assert_eq!(
            edit_item(&bundle_dir, "ki-a-000001", "confidence", "-0.1", "u", "r").unwrap_err().to_string(),
            "confidence must be between 0 and 1"
        );
        assert_eq!(
            edit_item(&bundle_dir, "ki-a-000001", "confidence", "nan", "u", "r").unwrap_err().to_string(),
            "confidence must be between 0 and 1"
        );
        assert_eq!(
            edit_item(&bundle_dir, "ki-a-000001", "confidence", "abc", "u", "r").unwrap_err().to_string(),
            "could not convert string to float: 'abc'"
        );
    }

    #[test]
    fn edit_confidence_negative_zero_and_infinity_bounds() {
        let dir = bundle_with_one_item("proposed");
        let bundle_dir = dir.path().join("bundle");
        let event = edit_item(&bundle_dir, "ki-a-000001", "confidence", "-0.0", "u", "r").unwrap();
        assert_eq!(event["new_value"]["confidence"], -0.0);
        assert_eq!(
            edit_item(&bundle_dir, "ki-a-000001", "confidence", "inf", "u", "r").unwrap_err().to_string(),
            "confidence must be between 0 and 1"
        );
    }

    #[test]
    fn approve_unknown_item_id_is_a_clear_error() {
        let dir = bundle_with_one_item("proposed");
        let bundle_dir = dir.path().join("bundle");
        let err = approve_item(&bundle_dir, "ki-does-not-exist", "u", "r").unwrap_err();
        assert_eq!(err.to_string(), "knowledge item not found: ki-does-not-exist");
    }

    #[test]
    fn missing_items_json_is_not_found() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        store::create_bundle(&bundle_dir, None).unwrap();
        let err = approve_item(&bundle_dir, "ki-a-000001", "u", "r").unwrap_err();
        assert_eq!(err.to_string(), "missing .sopkb/items.json");
    }

    #[test]
    fn append_review_event_rejects_invalid_action() {
        let dir = bundle_with_one_item("proposed");
        let bundle_dir = dir.path().join("bundle");
        let err = append_review_event(&bundle_dir, "ki-a-000001", "bogus", "u", "r", None, None).unwrap_err();
        assert_eq!(err.to_string(), "invalid review action: bogus");
    }

    #[test]
    fn review_action_regenerates_standard_reports() {
        let dir = bundle_with_one_item("proposed");
        let bundle_dir = dir.path().join("bundle");
        approve_item(&bundle_dir, "ki-a-000001", "u", "r").unwrap();
        assert!(bundle_dir.join("reports/review_summary.md").exists());
        assert!(bundle_dir.join("reports/freshness.md").exists());
    }

    #[test]
    fn missing_or_unrecognized_review_status_is_treated_as_mutable() {
        let dir = bundle_with_one_item("some-unknown-status");
        let bundle_dir = dir.path().join("bundle");
        let event = approve_item(&bundle_dir, "ki-a-000001", "u", "r").unwrap();
        assert_eq!(event["previous_value"]["review_status"], "some-unknown-status");
    }
}
