//! The five standard report renderers, called only from `validate_bundle`.
//! docs/port/port-mapping-c-review-export.md §4.1-4.5, plus `source_update_impact.md`
//! from the source-versioning subsystem (docs/port/CATCHUP_PLAN.md workstream 2).

use sopkb_core::error::Result;
use sopkb_core::store;
use std::collections::BTreeMap;
use std::path::Path;

/// Writes all five report files. `errors`/`warnings` are accepted for call-site
/// symmetry with the Python signature but never used -- each renderer re-reads its own
/// state from disk.
pub fn write_standard_reports(bundle_dir: &Path, _errors: &[String], _warnings: &[String]) -> Result<()> {
    let inventory = store::read_state_json(bundle_dir, "inventory.json", serde_json::json!({"sources": []}))?;
    let items = store::read_state_json(bundle_dir, "items.json", serde_json::json!([]))?;
    let reviews = store::read_state_json(bundle_dir, "reviews.json", serde_json::json!([]))?;
    let sections = store::read_state_json(bundle_dir, "sections.json", serde_json::json!([]))?;

    let reports_dir = bundle_dir.join("reports");
    std::fs::create_dir_all(&reports_dir)?;

    store::write_text_native(&reports_dir.join("freshness.md"), &render_freshness(&inventory, &items))?;
    store::write_text_native(&reports_dir.join("conflicts.md"), &render_conflicts(&items))?;
    store::write_text_native(
        &reports_dir.join("extraction_summary.md"),
        &render_extraction_summary(&sections, &items),
    )?;
    store::write_text_native(&reports_dir.join("review_summary.md"), &render_review_summary(&items, &reviews))?;
    store::write_text_native(
        &reports_dir.join("source_update_impact.md"),
        &render_source_update_impact(&inventory, &items),
    )?;
    Ok(())
}

/// `reports/source_update_impact.md` -- the human-readable face of the lifecycle state:
/// how many source versions exist and in what state, how many knowledge items sit in
/// each lifecycle status, and a line per source version.
///
/// Ground truth for the exact layout (heading text, blank lines, backticks around the
/// path, the trailing newline) is
/// `fixtures/cases/weird-headings-md/expected-python/bundle/reports/source_update_impact.md`.
pub fn render_source_update_impact(inventory: &serde_json::Value, items: &serde_json::Value) -> String {
    let sources: Vec<&serde_json::Value> = inventory["sources"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let versions: Vec<&serde_json::Value> = sources
        .iter()
        .flat_map(|s| s["versions"].as_array().map(|a| a.iter().collect::<Vec<_>>()).unwrap_or_default())
        .collect();
    let version_status = |v: &serde_json::Value| v["status"].as_str().unwrap_or("").to_string();
    let count_with_status = |status: &str| versions.iter().filter(|v| version_status(v) == status).count();

    // `BTreeMap` so the per-status tail below comes out in `sorted(lifecycle_counts)`
    // order without an explicit sort.
    let mut lifecycle_counts: BTreeMap<String, usize> = BTreeMap::new();
    for item in items.as_array().into_iter().flatten() {
        let status = item["lifecycle_status"].as_str().unwrap_or("active").to_string();
        *lifecycle_counts.entry(status).or_insert(0) += 1;
    }
    let count_of = |status: &str| lifecycle_counts.get(status).copied().unwrap_or(0);

    let mut lines: Vec<String> = vec!["# Source Update Impact".to_string(), String::new()];
    lines.push(format!("Sources: {}", sources.len()));
    lines.push(format!("Source versions: {}", versions.len()));
    lines.push(format!("Active source versions: {}", count_with_status("active")));
    lines.push(format!("Superseded source versions: {}", count_with_status("superseded")));
    lines.push(format!("Retired source versions: {}", count_with_status("retired")));
    lines.push(String::new());
    lines.push("## Knowledge Lifecycle".to_string());
    lines.push(format!("Active knowledge items: {}", count_of("active")));
    lines.push(format!("Superseded knowledge items: {}", count_of("superseded")));
    lines.push(format!("Retired knowledge items: {}", count_of("retired")));
    lines.push(format!("Conflicted knowledge items: {}", count_of("conflicted")));
    lines.push(String::new());
    if lifecycle_counts.is_empty() {
        lines.push("- No knowledge items found.".to_string());
    } else {
        for (status, count) in &lifecycle_counts {
            lines.push(format!("- {status}: {count}"));
        }
    }
    lines.push(String::new());
    lines.push("## Source Versions".to_string());
    if versions.is_empty() {
        lines.push("- No source versions found.".to_string());
    } else {
        let mut sorted: Vec<&serde_json::Value> = versions.clone();
        sorted.sort_by_key(|v| {
            (v["source_id"].as_str().unwrap_or("").to_string(), v["version_number"].as_i64().unwrap_or(0))
        });
        for version in sorted {
            lines.push(format!(
                "- {}: {} `{}`",
                version["source_version_id"].as_str().unwrap_or("None"),
                version["status"].as_str().unwrap_or("unknown"),
                version["original_path"].as_str().unwrap_or("")
            ));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

/// Counts only (truthy `stale_after`); no dates, no "today" comparison.
/// `item.get("metadata", {}).get("stale_after")` in Python breaks if `metadata` is
/// present but `null` -- reproduced here via `.get("metadata").and_then(|m| m.as_object())`,
/// which likewise yields nothing (not a crash) for a null/absent/non-object metadata.
pub fn render_freshness(inventory: &serde_json::Value, items: &serde_json::Value) -> String {
    let stale_sources = inventory["sources"]
        .as_array()
        .map(|arr| arr.iter().filter(|s| is_truthy(&s["stale_after"])).count())
        .unwrap_or(0);
    let stale_items = items
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|i| i.get("metadata").and_then(|m| m.as_object()).and_then(|m| m.get("stale_after")).is_some_and(is_truthy))
                .count()
        })
        .unwrap_or(0);
    format!(
        "# Freshness Report\n\nSources with freshness metadata: {stale_sources}\nKnowledge items with freshness metadata: {stale_items}\n"
    )
}

fn is_truthy(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Array(a) => !a.is_empty(),
        serde_json::Value::Object(o) => !o.is_empty(),
    }
}

/// One warning line per offending item (the 2nd+ member of a `(subject, predicate)`
/// group whose object differs from the IMMEDIATELY PRECEDING object for that key --
/// `seen[key]` is overwritten every time, so A->B->A warns on items 2 and 3, while
/// A->A->B warns only on item 3). `previous` must be truthy (an empty-string previous
/// object suppresses the check).
pub fn render_conflicts(items: &serde_json::Value) -> String {
    let mut seen: std::collections::HashMap<(String, String), String> = std::collections::HashMap::new();
    let mut conflicts = Vec::new();
    if let Some(arr) = items.as_array() {
        for item in arr {
            let subject = item["subject"].as_str().unwrap_or("").to_string();
            let predicate = item["predicate"].as_str().unwrap_or("").to_string();
            let object = item["object"].as_str().unwrap_or("").to_string();
            let key = (subject.clone(), predicate.clone());
            if let Some(previous) = seen.get(&key) {
                if !previous.is_empty() && previous != &object {
                    let id = item["id"].as_str().unwrap_or("");
                    conflicts.push(format!("- Candidate conflict for `{subject}` / `{predicate}` involving `{id}`"));
                }
            }
            seen.insert(key, object);
        }
    }
    let mut lines: Vec<String> = vec!["# Conflict Report".to_string(), String::new()];
    if conflicts.is_empty() {
        lines.push("- No candidate conflicts detected.".to_string());
    } else {
        lines.extend(conflicts);
    }
    lines.push(String::new());
    lines.join("\n")
}

pub fn render_extraction_summary(sections: &serde_json::Value, items: &serde_json::Value) -> String {
    let section_count = sections.as_array().map(|a| a.len()).unwrap_or(0);
    let item_count = items.as_array().map(|a| a.len()).unwrap_or(0);
    format!("# Extraction Summary\n\nSections: {section_count}\nKnowledge items: {item_count}\n")
}

/// Status keys are ASCII-sorted (Python `sorted(counts)` on strings, matching Rust's
/// `BTreeMap<String,_>`/`sort()` byte order). `Review events` counts every event,
/// including comments (which never show up as a status here, since statuses only ever
/// come from `items`, not `reviews`).
pub fn render_review_summary(items: &serde_json::Value, reviews: &serde_json::Value) -> String {
    let mut counts: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    if let Some(arr) = items.as_array() {
        for item in arr {
            let status = item["review_status"].as_str().unwrap_or("unknown").to_string();
            *counts.entry(status).or_insert(0) += 1;
        }
    }
    let mut lines: Vec<String> = vec!["# Review Summary".to_string(), String::new()];
    if counts.is_empty() {
        lines.push("- No knowledge items found.".to_string());
    } else {
        for (status, count) in &counts {
            lines.push(format!("- {status}: {count}"));
        }
    }
    lines.push(String::new());
    let review_count = reviews.as_array().map(|a| a.len()).unwrap_or(0);
    lines.push(format!("Review events: {review_count}"));
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn freshness_counts_only_truthy_stale_after() {
        let inventory = json!({"sources": [{"id": "a", "stale_after": "2026-01-01"}, {"id": "b"}]});
        let items = json!([{"metadata": {"stale_after": true}}, {"metadata": {}}]);
        let out = render_freshness(&inventory, &items);
        assert_eq!(out, "# Freshness Report\n\nSources with freshness metadata: 1\nKnowledge items with freshness metadata: 1\n");
    }

    #[test]
    fn conflicts_clean_bundle() {
        let items = json!([{"id": "ki-1", "subject": "A", "predicate": "requires", "object": "X"}]);
        let out = render_conflicts(&items);
        assert_eq!(out, "# Conflict Report\n\n- No candidate conflicts detected.\n");
    }

    #[test]
    fn conflicts_a_b_a_warns_on_items_2_and_3() {
        let items = json!([
            {"id": "ki-1", "subject": "S", "predicate": "P", "object": "A"},
            {"id": "ki-2", "subject": "S", "predicate": "P", "object": "B"},
            {"id": "ki-3", "subject": "S", "predicate": "P", "object": "A"},
        ]);
        let out = render_conflicts(&items);
        assert!(out.contains("involving `ki-2`"));
        assert!(out.contains("involving `ki-3`"));
    }

    #[test]
    fn conflicts_a_a_b_warns_only_on_item_3() {
        let items = json!([
            {"id": "ki-1", "subject": "S", "predicate": "P", "object": "A"},
            {"id": "ki-2", "subject": "S", "predicate": "P", "object": "A"},
            {"id": "ki-3", "subject": "S", "predicate": "P", "object": "B"},
        ]);
        let out = render_conflicts(&items);
        assert!(!out.contains("ki-2"));
        assert!(out.contains("involving `ki-3`"));
    }

    #[test]
    fn review_summary_matches_pinned_example() {
        let items = json!([
            {"review_status": "approved"},
            {"review_status": "proposed"}, {"review_status": "proposed"}, {"review_status": "proposed"},
            {"review_status": "proposed"}, {"review_status": "proposed"}, {"review_status": "proposed"},
            {"review_status": "proposed"}, {"review_status": "proposed"}, {"review_status": "proposed"},
            {"review_status": "proposed"}, {"review_status": "proposed"}, {"review_status": "proposed"},
        ]);
        let reviews = json!([{"id": "review-000001"}]);
        let out = render_review_summary(&items, &reviews);
        assert!(out.contains("- approved: 1"));
        assert!(out.contains("- proposed: 12"));
        assert!(out.contains("Review events: 1"));
    }

    #[test]
    fn review_summary_empty_items() {
        let out = render_review_summary(&json!([]), &json!([]));
        assert!(out.contains("- No knowledge items found."));
        assert!(out.contains("Review events: 0"));
    }
}
