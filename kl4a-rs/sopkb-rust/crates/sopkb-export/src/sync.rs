//! `sync_okf_bundle` and the destructive `reset_okf_documents` step.
//! docs/port/port-mapping-c-review-export.md §3.4-3.5.

use crate::okf_docs;
use sopkb_core::error::Result;
use sopkb_core::store;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// `reset_okf_documents` (§3.5). **Deliberately private** (not even `pub(crate)`) --
/// the ONLY caller is `sync_okf_bundle` in this module, immediately after
/// `migrate_legacy_state`. P-X3 [PRESERVE, enforced structurally]: reordering these
/// two calls would let this function destroy 7 of the 9 legacy state files (they live
/// under `knowledge/`, which this wipes recursively) before they are ever migrated
/// into `.sopkb/`. Rust's privacy makes that reordering a compile error anywhere
/// outside this file, rather than a runtime data-loss bug (G12/G-A22).
///
/// P-X2 [PRESERVE -- test it explicitly]: `sources/*.md` is glob-deleted
/// NON-recursively (top-level only), so `sources/originals/` and `sources/normalized/`
/// survive untouched. The other nine directories are wiped completely and recursively
/// regardless of extension. Getting this backwards deletes the original documents.
fn reset_okf_documents(bundle_dir: &Path) -> Result<()> {
    for filename in ["index.md", "log.md"] {
        let path = bundle_dir.join(filename);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
    }

    let sources_dir = bundle_dir.join("sources");
    std::fs::create_dir_all(&sources_dir)?;
    for entry in std::fs::read_dir(&sources_dir)? {
        let entry = entry?;
        let path = entry.path();
        let is_md = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("md"));
        if is_md && path.is_file() {
            std::fs::remove_file(&path)?;
        }
    }

    for relative in ["sections", "concepts", "knowledge", "relations", "rules", "evidence", "tasks", "references", "authored"] {
        reset_directory(&bundle_dir.join(relative))?;
    }
    Ok(())
}

fn reset_directory(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            std::fs::remove_dir_all(&p)?;
        } else {
            std::fs::remove_file(&p)?;
        }
    }
    Ok(())
}

/// `group_by(items, key)`: groups by `item[key]`. P-X9 [DECIDE, lean FIX -- applied
/// per PORT_PLAN.md §6.6's explicit "Apply P-X9" instruction]: real Python indexes
/// `item[key]` directly, which raises `KeyError` and aborts the ENTIRE sync when a
/// section lacks `source_id` or an item lacks `source_id`/`section_id` -- even though
/// `validate_bundle` treats the identical condition as a mere reported error and keeps
/// going. This port skips such a record from the grouping instead of crashing
/// (matching the recommended fix), so a bundle with one malformed record can still be
/// synced and inspected rather than refusing to export at all. Unreachable for any
/// bundle the current Python `sync_okf_bundle` can already process, including the real
/// reference bundle -- if it weren't well-formed, Python's own sync would already be
/// crashing on it today.
fn group_by(items: &[Value], key: &str) -> HashMap<String, Vec<Value>> {
    let mut grouped: HashMap<String, Vec<Value>> = HashMap::new();
    for item in items {
        if let Some(k) = item.get(key).and_then(|v| v.as_str()) {
            grouped.entry(k.to_string()).or_default().push(item.clone());
        }
    }
    grouped
}

/// `latest_reviews(reviews)`: last event per item id, in FILE ORDER -- NOT sorted by
/// timestamp, and includes `commented` events (G2). A `commented` event after an
/// approval makes that commenter the `verified.actor` of the knowledge doc.
fn latest_reviews(reviews: &[Value]) -> HashMap<String, Value> {
    let mut latest = HashMap::new();
    for review in reviews {
        if let Some(id) = review.get("knowledge_item_id").and_then(|v| v.as_str()) {
            latest.insert(id.to_string(), review.clone());
        }
    }
    latest
}

/// `sync_okf_bundle(bundle_dir) -> {"type":"okf_native","path":"."}` (§3.4). The OKF
/// bundle is written IN PLACE (`export_dir == bundle_dir`): a full destroy-and-rebuild
/// of nine directories plus `index.md`/`log.md`, not an incremental update (G5).
pub fn sync_okf_bundle(bundle_dir: &Path) -> Result<Value> {
    let export_dir = bundle_dir;
    store::migrate_legacy_state(bundle_dir)?;
    // Source-version migration runs AFTER the legacy-state relocation above and BEFORE
    // the reset below, and the order is load-bearing in both directions: it reads
    // `.sopkb/inventory.json`, which `migrate_legacy_state` may only just have created
    // from `sources/inventory.json`; and the OKF documents rebuilt below embed
    // `source_version_id` / `lifecycle_status`, which a pre-versioning bundle does not
    // have until this runs. Without it, syncing a legacy bundle emits `null` version
    // fields into every source/section/knowledge/evidence document
    // (caught by `fixtures/cases/legacy-layout`).
    sopkb_core::lifecycle::migrate_source_version_state(bundle_dir)?;
    reset_okf_documents(export_dir)?;

    let manifest = store::load_manifest(bundle_dir)?;
    let inventory = store::read_state_json(bundle_dir, "inventory.json", serde_json::json!({"sources": []}))?;
    let sections: Vec<Value> = store::read_state_json(bundle_dir, "sections.json", serde_json::json!([]))?.as_array().cloned().unwrap_or_default();
    let items: Vec<Value> = store::read_state_json(bundle_dir, "items.json", serde_json::json!([]))?.as_array().cloned().unwrap_or_default();
    let reviews: Vec<Value> = store::read_state_json(bundle_dir, "reviews.json", serde_json::json!([]))?.as_array().cloned().unwrap_or_default();
    let sources: Vec<Value> = inventory.get("sources").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    let mut source_by_id: HashMap<String, Value> = HashMap::new();
    for source in &sources {
        if let Some(id) = source.get("id").and_then(|v| v.as_str()) {
            source_by_id.insert(id.to_string(), source.clone()); // later wins on dup ids
        }
    }
    let sections_by_source = group_by(&sections, "source_id");
    let items_by_source = group_by(&items, "source_id");
    let items_by_section = group_by(&items, "section_id");
    let latest_review_by_item = latest_reviews(&reviews);

    okf_docs::write_root_index(export_dir, &manifest, &sources, &sections, &items)?;
    okf_docs::write_log(export_dir)?;

    let source_entries = okf_docs::write_source_docs(export_dir, &source_by_id, &sections_by_source, &items_by_source)?;
    let section_entries = okf_docs::write_section_docs(export_dir, bundle_dir, &sections, &source_by_id, &items_by_section)?;
    let evidence_entries = okf_docs::write_evidence_docs(export_dir, &items, &source_by_id)?;
    let rule_entries = okf_docs::write_rule_docs(export_dir, &items, &source_by_id)?;
    let relation_entries = okf_docs::write_relation_docs(export_dir, &items, &source_by_id)?;
    let knowledge_entries = okf_docs::write_knowledge_docs(export_dir, &items, &source_by_id, &latest_review_by_item)?;
    let concept_entries = okf_docs::write_concept_docs(export_dir, &items, &section_entries)?;
    let task_entries = okf_docs::write_task_docs(export_dir)?;
    let reference_entries = okf_docs::write_reference_docs(export_dir)?;
    let authored_entries = okf_docs::copy_authored_okf_docs(bundle_dir, export_dir)?;

    okf_docs::write_directory_index(export_dir, "sources", "SOP Sources", &source_entries)?;
    okf_docs::write_directory_index(export_dir, "sections", "SOP Sections", &section_entries)?;
    okf_docs::write_directory_index(export_dir, "evidence", "SOP Evidence", &evidence_entries)?;
    okf_docs::write_directory_index(export_dir, "rules", "SOP Decision Rules", &rule_entries)?;
    okf_docs::write_directory_index(export_dir, "relations", "Knowledge Relations", &relation_entries)?;
    okf_docs::write_directory_index(export_dir, "knowledge", "Knowledge Pieces", &knowledge_entries)?;
    okf_docs::write_directory_index(export_dir, "concepts", "Concepts", &concept_entries)?;
    okf_docs::write_directory_index(export_dir, "tasks", "Agent Task Contexts", &task_entries)?;
    okf_docs::write_directory_index(export_dir, "references", "References", &reference_entries)?;
    okf_docs::write_directory_index(export_dir, "authored", "LLM Authored Drafts", &authored_entries)?;

    Ok(serde_json::json!({"type": "okf_native", "path": "."}))
}
