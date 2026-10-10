//! `validate_bundle`. docs/port/port-mapping-a-core-data.md §3.6,
//! docs/port/port-mapping-c-review-export.md §4.6.
//!
//! Phase 5 filled in `sopkb_export::sync_okf_bundle` for real; the trailing sync call
//! below now calls straight through to it (previously a Phase-3 no-op stub). Per
//! PORT_PLAN.md §3.3, `sopkb-export` must never depend on `sopkb-review` -- this crate
//! depends on `sopkb-export`, never the reverse.

use sopkb_core::error::Result;
use sopkb_core::store;
use sopkb_core::validate_report::render_validation_report;
use sopkb_export::reports::write_standard_reports;
use std::path::Path;

const REVIEW_ACTIONS: &[&str] = &["approved", "rejected", "deferred", "commented", "edited"];
const REVIEW_STATUSES: &[&str] = &["proposed", "approved", "rejected", "deferred", "edited"];

/// `sopkb_export::sync_okf_bundle` returns the `{"type":"okf_native","path":"."}`
/// descriptor, which `validate_bundle` (like the real Python `validate.py`) discards.
fn sync_okf_bundle_stub(bundle_dir: &Path) -> Result<()> {
    sopkb_export::sync_okf_bundle(bundle_dir)?;
    Ok(())
}

/// Returns `(errors, warnings)`. On a manifest load failure, returns immediately with
/// NO side effects at all -- no reports written, `write_standard_reports` not called,
/// OKF sync not attempted (`reports/` may not even exist yet on a bundle this broken).
pub fn validate_bundle(bundle_dir: &Path) -> Result<(Vec<String>, Vec<String>)> {
    let manifest = match store::load_manifest(bundle_dir) {
        Ok(m) => m,
        Err(e) => return Ok((vec![e.to_string()], vec![])),
    };

    let mut errors = store::validate_manifest_fields(&manifest);
    let mut warnings: Vec<String> = Vec::new();

    for rel in store::REQUIRED_DIRS {
        if !bundle_dir.join(rel).is_dir() {
            errors.push(format!("missing required directory: {rel}"));
        }
    }

    let inventory = store::read_state_json(bundle_dir, "inventory.json", serde_json::Value::Null)?;
    if inventory.is_null() {
        warnings.push("missing .sopkb/inventory.json".to_string());
    } else {
        let inventory_ids = id_set(inventory["sources"].as_array());
        let manifest_ids = manifest_source_id_set(&manifest);
        if inventory_ids != manifest_ids {
            errors.push("manifest sources do not match inventory sources".to_string());
        }
        for w in inventory["warnings"].as_array().into_iter().flatten() {
            let path = string_or_none(&w["path"]);
            let warning = string_or_none(&w["warning"]);
            warnings.push(format!("inventory warning: {path} - {warning}"));
        }
        for source in inventory["sources"].as_array().into_iter().flatten() {
            let original_path = source["original_path"].as_str().unwrap_or("");
            if !original_path.is_empty() && !bundle_dir.join(original_path).exists() {
                errors.push(format!("missing original source: {original_path}"));
            }
            let normalized_path = source["normalized_path"].as_str().unwrap_or("");
            if source["parse_status"] == "normalized"
                && !normalized_path.is_empty()
                && !bundle_dir.join(normalized_path).exists()
            {
                errors.push(format!("missing normalized source: {normalized_path}"));
            }
            let source_id = string_or_none(&source["id"]);
            for w in source["warnings"].as_array().into_iter().flatten() {
                warnings.push(format!("{source_id}: {}", string_or_none(w)));
            }
        }
    }

    let items = store::read_state_json(bundle_dir, "items.json", serde_json::json!([]))?;
    let item_ids = id_set(items.as_array());
    for item in items.as_array().into_iter().flatten() {
        let id = string_or_none(&item["id"]);
        let status = item["review_status"].as_str().unwrap_or("");
        if !REVIEW_STATUSES.contains(&status) {
            errors.push(format!("{id}: invalid review status"));
        }
        if !is_truthy(&item["source_id"]) {
            errors.push(format!("{id}: missing source id"));
        }
        if !is_truthy(&item["section_id"]) {
            errors.push(format!("{id}: missing section id"));
        }
        if !is_truthy(&item["source_text"]) && item["span_status"] != "unresolved" {
            warnings.push(format!("{id}: missing source text"));
        }
        let confidence = item.get("confidence").and_then(|c| c.as_f64()).unwrap_or(1.0);
        if confidence < 0.7 {
            warnings.push(format!("{id}: low confidence"));
        }
    }

    let reviews = store::read_state_json(bundle_dir, "reviews.json", serde_json::json!([]))?;
    for review in reviews.as_array().into_iter().flatten() {
        let rid = review.get("id").and_then(|v| v.as_str()).unwrap_or("unknown-review").to_string();
        let knowledge_item_id = review["knowledge_item_id"].as_str().unwrap_or("");
        if !item_ids.contains(knowledge_item_id) {
            errors.push(format!("{rid}: review references unknown knowledge item"));
        }
        let action = review["action"].as_str().unwrap_or("");
        if !REVIEW_ACTIONS.contains(&action) {
            errors.push(format!("{rid}: invalid review action"));
        }
        if !is_truthy(&review["reviewer"]) {
            errors.push(format!("{rid}: missing reviewer"));
        }
        if !is_truthy(&review["timestamp"]) {
            errors.push(format!("{rid}: missing timestamp"));
        }
    }

    store::write_json(
        &bundle_dir.join("reports").join("validation.json"),
        &serde_json::json!({"errors": errors, "warnings": warnings}),
    )?;
    store::write_text_native(
        &bundle_dir.join("reports").join("validation.md"),
        &render_validation_report(&errors, &warnings),
    )?;
    write_standard_reports(bundle_dir, &errors, &warnings)?;
    sync_okf_bundle_stub(bundle_dir)?;

    Ok((errors, warnings))
}

fn is_truthy(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        serde_json::Value::Array(a) => !a.is_empty(),
        serde_json::Value::Object(o) => !o.is_empty(),
    }
}

fn string_or_none(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "None".to_string(),
        other => other.to_string(),
    }
}

fn id_set(items: Option<&Vec<serde_json::Value>>) -> std::collections::HashSet<String> {
    items
        .into_iter()
        .flatten()
        .map(|i| i["id"].as_str().map(|s| s.to_string()).unwrap_or_else(|| "None".to_string()))
        .collect()
}

fn manifest_source_id_set(manifest: &sopkb_fmt::OrderedMap<sopkb_fmt::YamlValue>) -> std::collections::HashSet<String> {
    manifest
        .get("sources")
        .and_then(|s| s.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|entry| entry.as_mapping())
                .map(|m| m.get("id").and_then(|v| v.as_str()).unwrap_or("None").to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn validate_bundle_missing_manifest_returns_single_error_no_side_effects() {
        let dir = tempdir().unwrap();
        let (errors, warnings) = validate_bundle(dir.path()).unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].starts_with("Missing manifest: "));
        assert!(warnings.is_empty());
        assert!(!dir.path().join("reports").exists(), "no reports dir should be created");
    }

    #[test]
    fn validate_bundle_fresh_bundle_has_missing_inventory_warning() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        sopkb_core::store::create_bundle(&bundle_dir, None).unwrap();
        let (errors, warnings) = validate_bundle(&bundle_dir).unwrap();
        assert!(errors.is_empty(), "fresh bundle should have zero errors: {errors:?}");
        assert!(warnings.contains(&"missing .sopkb/inventory.json".to_string()));
        assert!(bundle_dir.join("reports/validation.json").exists());
        assert!(bundle_dir.join("reports/freshness.md").exists());
    }

    #[test]
    fn validate_bundle_flags_missing_required_directory() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        sopkb_core::store::create_bundle(&bundle_dir, None).unwrap();
        std::fs::remove_dir_all(bundle_dir.join(".sopkb/cache")).unwrap();
        let (errors, _warnings) = validate_bundle(&bundle_dir).unwrap();
        assert!(errors.contains(&"missing required directory: .sopkb/cache".to_string()));
    }
}
