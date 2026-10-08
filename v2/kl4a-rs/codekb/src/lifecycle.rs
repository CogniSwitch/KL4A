//! Port of `kl4a/codekb/lifecycle.py`.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use serde_json::{Map, Value};

// WIRING: `read_json(path: &Path, default: Value) -> Value` was confirmed via
// MCP against kl4a/codekb/state.py -- it tolerates a missing/torn/empty file
// by returning `default` and never raises, so it is not wrapped in `Result`
// here.
use crate::state::{read_json, STATE_DIR};

/// Mirrors `kl4a.codekb.lifecycle.REVIEW_STATE_FILENAME`.
pub const REVIEW_STATE_FILENAME: &str = "code_reviews.json";

/// Mirrors `read_review_state`: the reviewer decisions recorded by the
/// Workbench review UI.
pub fn read_review_state(bundle_dir: &Path) -> Value {
    let mut default = Map::new();
    default.insert("status".to_string(), Value::Object(Map::new()));
    default.insert("events".to_string(), Value::Array(Vec::new()));
    read_json(
        &bundle_dir.join(STATE_DIR).join(REVIEW_STATE_FILENAME),
        Value::Object(default),
    )
}

/// Mirrors `merge_code_items`.
///
/// Merges a fresh mining pass into prior state without losing human review.
/// Three things have to survive a re-mine:
/// - a reviewer's approve/reject/defer decision on a claim that still exists
/// - claims whose symbol was parsed from a source version that is no longer
///   current, which become `superseded` rather than silently disappearing
/// - claims whose symbol is gone entirely, which become `retired`
///
/// Mirrors `sopkb.knowledge_lifecycle.merge_mined_items` on the SOP side,
/// keyed on symbol id and source version instead of section and source id.
pub fn merge_code_items(
    bundle_dir: &Path,
    existing_items: &[Value],
    new_items: &[Value],
    symbols: &[Value],
) -> Result<Vec<Value>> {
    let review_status_state = read_review_state(bundle_dir);
    let review_status = review_status_state
        .get("status")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let current_versions: HashSet<String> = symbols
        .iter()
        .filter_map(|s| s.get("source_version_id").and_then(Value::as_str))
        .map(|s| s.to_string())
        .collect();
    let current_symbol_ids: HashSet<String> = symbols
        .iter()
        .filter_map(|s| s.get("id").and_then(Value::as_str))
        .map(|s| s.to_string())
        .collect();

    let mut new_items: Vec<Value> = new_items.to_vec();
    let mut new_by_id: HashSet<String> = HashSet::new();
    let mut new_ids_by_symbol: HashMap<String, Vec<String>> = HashMap::new();
    for item in &mut new_items {
        if let Some(obj) = item.as_object_mut() {
            obj.entry("supersedes")
                .or_insert_with(|| Value::Array(Vec::new()));
            obj.entry("superseded_by")
                .or_insert_with(|| Value::Array(Vec::new()));
        }
        let item_id = item
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        new_by_id.insert(item_id.clone());
        if let Some(sids) = item.get("symbols").and_then(Value::as_array) {
            for sid in sids {
                new_ids_by_symbol
                    .entry(value_to_key_string(sid))
                    .or_default()
                    .push(item_id.clone());
            }
        }
    }

    let mut merged: Vec<Value> = Vec::new();
    let mut superseded_ids_by_symbol: HashMap<String, Vec<String>> = HashMap::new();

    for item in existing_items {
        let mut item = item.clone();
        let item_id = item
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if new_by_id.contains(&item_id) {
            // Re-mined this pass; the fresh record wins and carries review
            // state forward.
            continue;
        }

        let item_symbols: Vec<String> = item
            .get("symbols")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(value_to_key_string).collect())
            .unwrap_or_default();
        let item_version: Option<String> = item
            .get("code")
            .and_then(|c| c.get("source_version_id"))
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());
        let lifecycle_status = item
            .get("lifecycle_status")
            .and_then(Value::as_str)
            .unwrap_or("active")
            .to_string();

        if lifecycle_status != "active" {
            merged.push(item);
            continue;
        }

        if !item_symbols
            .iter()
            .any(|sid| current_symbol_ids.contains(sid))
        {
            set_str_field(&mut item, "lifecycle_status", "retired");
        } else if let Some(version) = &item_version {
            if !current_versions.contains(version) {
                set_str_field(&mut item, "lifecycle_status", "superseded");
                let mut replacements: BTreeSet<String> = BTreeSet::new();
                for sid in &item_symbols {
                    if let Some(reps) = new_ids_by_symbol.get(sid) {
                        replacements.extend(reps.iter().cloned());
                    }
                }
                if let Some(obj) = item.as_object_mut() {
                    obj.insert(
                        "superseded_by".to_string(),
                        Value::Array(replacements.into_iter().map(Value::String).collect()),
                    );
                }
                for sid in &item_symbols {
                    superseded_ids_by_symbol
                        .entry(sid.clone())
                        .or_default()
                        .push(item_id.clone());
                }
            } else {
                set_str_field(&mut item, "lifecycle_status", "retired");
            }
        } else {
            set_str_field(&mut item, "lifecycle_status", "retired");
        }
        merged.push(item);
    }

    for item in &mut new_items {
        let item_symbols: Vec<String> = item
            .get("symbols")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(value_to_key_string).collect())
            .unwrap_or_default();

        let mut supersedes: BTreeSet<String> = item
            .get("supersedes")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        for sid in &item_symbols {
            if let Some(sups) = superseded_ids_by_symbol.get(sid) {
                supersedes.extend(sups.iter().cloned());
            }
        }
        if let Some(obj) = item.as_object_mut() {
            obj.insert(
                "supersedes".to_string(),
                Value::Array(supersedes.into_iter().map(Value::String).collect()),
            );
        }

        let item_id = item.get("id").and_then(Value::as_str).unwrap_or("");
        if let Some(decision) = review_status.get(item_id) {
            if is_truthy(decision) {
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("review_status".to_string(), decision.clone());
                    obj.insert("reviewed".to_string(), Value::Bool(true));
                }
            }
        }
    }

    merged.extend(new_items);
    Ok(merged)
}

/// Mirrors `lifecycle_summary`.
pub fn lifecycle_summary(items: &[Value]) -> HashMap<String, i64> {
    let mut summary: HashMap<String, i64> = HashMap::new();
    for item in items {
        let status = item
            .get("lifecycle_status")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("active")
            .to_string();
        *summary.entry(status).or_insert(0) += 1;
    }
    summary
}

fn set_str_field(value: &mut Value, key: &str, val: &str) {
    if let Some(obj) = value.as_object_mut() {
        obj.insert(key.to_string(), Value::String(val.to_string()));
    }
}

/// Python `str(symbol_id)` applied to entries of `item.get("symbols", [])`.
fn value_to_key_string(v: &Value) -> String {
    v.as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| v.to_string())
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}
