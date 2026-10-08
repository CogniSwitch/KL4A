//! `write_root_index`, `write_log`, the eight OKF document writers,
//! `write_directory_index`, `copy_authored_okf_docs`, `read_section_text`.
//! docs/port/port-mapping-c-review-export.md §3.8-3.20.

use crate::okf_meta::{decision_rule_to_yaml, decision_rules_with_raw, okf_metadata, okf_source_ref, okf_status_for_review, relation_to_yaml, write_okf_doc, OKF_VERSION};
use crate::okf_paths::*;
use sopkb_core::error::{Result, SopkbError};
use sopkb_core::store;
use sopkb_derive::concepts::concept_for_label;
use sopkb_derive::relations::{evidence_id_for_item, relation_id_for_item};
use sopkb_fmt::{OrderedMap, YamlValue};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// `{title, path, description}` for a directory-index entry (§3.10 etc.).
#[derive(Clone, Debug)]
pub struct DirEntry {
    pub title: String,
    pub path: String,
    pub description: String,
}

fn s(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_string()
}

fn sorted_by_id<'a>(items: impl IntoIterator<Item = &'a Value>) -> Vec<&'a Value> {
    let mut v: Vec<&Value> = items.into_iter().collect();
    v.sort_by(|a, b| a["id"].as_str().unwrap_or("").cmp(b["id"].as_str().unwrap_or("")));
    v
}

/// Python truthiness for a JSON value (`if section.get("ordinal"):` etc.).
fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `write_root_index` (§3.8): a hardcoded two-line frontmatter (`okf_version: "0.2"`,
/// double-quoted -- the ONE place in the whole OKF tree that differs from every other
/// `yaml.safe_dump`-produced value), never `write_okf_doc`.
pub fn write_root_index(export_dir: &Path, manifest: &OrderedMap<YamlValue>, sources: &[Value], sections: &[Value], items: &[Value]) -> Result<()> {
    let title = manifest.get("title").and_then(|v| v.as_str()).ok_or_else(|| SopkbError::Value("'title'".to_string()))?;
    let body = [
        format!("# {title}"),
        String::new(),
        "This OKF bundle exposes source material, normalized sections, reviewed knowledge pieces, RDF-compatible Knowledge Relations, evidence, and agent task contexts.".to_string(),
        String::new(),
        "## Contents".to_string(),
        String::new(),
        "- [Sources](sources/index.md)".to_string(),
        "- [Sections](sections/index.md)".to_string(),
        "- [Concepts](concepts/index.md)".to_string(),
        "- [Knowledge Pieces](knowledge/index.md)".to_string(),
        "- [Knowledge Relations](relations/index.md)".to_string(),
        "- [Decision Rules](rules/index.md)".to_string(),
        "- [Evidence](evidence/index.md)".to_string(),
        "- [Agent Tasks](tasks/index.md)".to_string(),
        "- [Agent Guide](references/agent-guide.md)".to_string(),
        "- [LLM Authored Drafts](authored/index.md)".to_string(),
        String::new(),
        "## Bundle Summary".to_string(),
        String::new(),
        format!("- Sources: {}", sources.len()),
        format!("- Sections: {}", sections.len()),
        format!("- Knowledge pieces: {}", items.len()),
        String::new(),
    ]
    .join("\n");
    let text = format!("---\nokf_version: \"{OKF_VERSION}\"\n---\n{body}");
    store::write_text_native(&export_dir.join("index.md"), &text)
}

/// `write_log` (§3.9): rewritten from scratch every sync -- not a history, always
/// exactly one dated entry for the last sync date. No frontmatter.
pub fn write_log(export_dir: &Path) -> Result<()> {
    let today = store::today();
    let body =
        ["# Bundle Log".to_string(), String::new(), format!("## {today}"), String::new(), "- Refreshed OKF-native SOP Knowledge Bundle.".to_string(), String::new()].join("\n");
    store::write_text_native(&export_dir.join("log.md"), &body)
}

/// P-X8 [FIX, per PORT_PLAN.md §6.6's explicit "Apply" list]: the real Python
/// `write_section_docs` falls back to `{"title": section["source_id"]}` (no `id` key)
/// when the section's `source_id` isn't in the inventory, which crashes
/// `source_doc_path(source)` with a `KeyError` -- while the knowledge/evidence/rule/
/// relation writers use a two-key `{"id":..., "title":...}` fallback and survive. This
/// port uses the same two-key fallback everywhere, eliminating the asymmetry rather
/// than reproducing the crash. See DEVIATIONS.md.
fn source_or_fallback<'a>(source_by_id: &'a HashMap<String, Value>, source_id: &str) -> std::borrow::Cow<'a, Value> {
    match source_by_id.get(source_id) {
        Some(v) => std::borrow::Cow::Borrowed(v),
        None => std::borrow::Cow::Owned(serde_json::json!({"id": source_id, "title": source_id})),
    }
}

/// `write_source_docs` (§3.10). Iterates `sorted(source_by_id.values(), key=id)`.
pub fn write_source_docs(
    export_dir: &Path,
    source_by_id: &HashMap<String, Value>,
    sections_by_source: &HashMap<String, Vec<Value>>,
    items_by_source: &HashMap<String, Vec<Value>>,
) -> Result<Vec<DirEntry>> {
    let mut sources: Vec<&Value> = source_by_id.values().collect();
    sources.sort_by(|a, b| a["id"].as_str().unwrap_or("").cmp(b["id"].as_str().unwrap_or("")));
    let mut entries = Vec::new();
    for source in sources {
        let source_id = s(source, "id");
        let rel_path = source_doc_path(source);
        let section_links: Vec<String> = sections_by_source
            .get(&source_id)
            .into_iter()
            .flatten()
            .map(|sec| bullet_link(&rel_path, &section_doc_path(sec), sec["heading"].as_str().unwrap_or("")))
            .collect();
        let knowledge_links: Vec<String> =
            items_by_source.get(&source_id).into_iter().flatten().map(|it| bullet_link(&rel_path, &knowledge_doc_path(it), it["subject"].as_str().unwrap_or(""))).collect();
        let mut metadata = okf_metadata(
            "SOP Source",
            &s(source, "title"),
            &format!("Inventoried {} source document.", s(source, "type")),
            &source_resource_for_okf(source),
            &["source", &s(source, "type")],
            None,
        );
        let mut sopkb = OrderedMap::new();
        sopkb.insert("source_id", YamlValue::Scalar(source_id.clone()));
        // The version block, ahead of the single-version legacy fields below it (which
        // still describe the ACTIVE version, so an old consumer keeps working).
        // `lifecycle_status` here mirrors the record's `status`, defaulting to
        // "active" -- note the key is renamed on the way out, matching the reference
        // implementation.
        sopkb.insert("source_version_id", opt_scalar(source, "source_version_id"));
        sopkb.insert("active_version_id", opt_scalar(source, "active_version_id"));
        sopkb.insert("version_number", opt_number(source, "version_number"));
        sopkb.insert(
            "lifecycle_status",
            YamlValue::Scalar(source.get("status").and_then(|v| v.as_str()).unwrap_or("active").to_string()),
        );
        sopkb.insert(
            "versions",
            YamlValue::Sequence(
                source
                    .get("versions")
                    .and_then(|v| v.as_array())
                    .into_iter()
                    .flatten()
                    .map(crate::okf_meta::json_to_yaml)
                    .collect(),
            ),
        );
        sopkb.insert("checksum", opt_scalar(source, "checksum"));
        sopkb.insert("original_path", opt_scalar(source, "original_path"));
        sopkb.insert("normalized_path", opt_scalar(source, "normalized_path"));
        sopkb.insert("mime_type", opt_scalar(source, "mime_type"));
        sopkb.insert("size_bytes", opt_number(source, "size_bytes"));
        metadata.insert("sopkb", YamlValue::Mapping(sopkb));
        if source.get("stale_after").is_some_and(is_truthy) {
            metadata.insert("stale_after", YamlValue::Scalar(s(source, "stale_after")));
        }
        let body = [
            format!("# {}", s(source, "title")),
            String::new(),
            "## Sections".to_string(),
            String::new(),
            or_placeholder(section_links, "- No normalized sections.").join("\n"),
            String::new(),
            "## Derived Knowledge".to_string(),
            String::new(),
            or_placeholder(knowledge_links, "- No knowledge pieces mined.").join("\n"),
            String::new(),
        ]
        .join("\n");
        write_okf_doc(export_dir, &rel_path, &metadata, &body)?;
        entries.push(DirEntry { title: s(source, "title"), path: rel_path, description: s(source, "type") });
    }
    Ok(entries)
}

/// `opt_scalar`: `.get(key)` (not truthy) -- `null` when absent, matching Python's
/// `source.get("checksum")` embedding `None` -> YAML `null` for an absent key.
/// `item.get("lifecycle_status", "active")` -- an absent or empty value reads as
/// `"active"`, so a pre-versioning item renders as live knowledge rather than as
/// something with no lifecycle at all.
fn lifecycle_status_of(item: &Value) -> String {
    match item.get("lifecycle_status").and_then(|v| v.as_str()) {
        Some(status) if !status.is_empty() => status.to_string(),
        _ => "active".to_string(),
    }
}

fn opt_scalar(v: &Value, key: &str) -> YamlValue {
    match v.get(key) {
        Some(Value::String(s)) => YamlValue::Scalar(s.clone()),
        Some(Value::Null) | None => YamlValue::Raw("null".to_string()),
        Some(other) => crate::okf_meta::json_to_yaml(other),
    }
}

fn opt_number(v: &Value, key: &str) -> YamlValue {
    match v.get(key) {
        Some(Value::Null) | None => YamlValue::Raw("null".to_string()),
        Some(other) => crate::okf_meta::json_to_yaml(other),
    }
}

/// `write_section_docs` (§3.11). Iterates `sorted(sections, key=id)`.
pub fn write_section_docs(export_dir: &Path, bundle_dir: &Path, sections: &[Value], source_by_id: &HashMap<String, Value>, items_by_section: &HashMap<String, Vec<Value>>) -> Result<Vec<DirEntry>> {
    let mut entries = Vec::new();
    for section in sorted_by_id(sections) {
        let source_id = s(section, "source_id");
        let source = source_or_fallback(source_by_id, &source_id);
        let rel_path = section_doc_path(section);
        let source_rel = source_doc_path(&source);
        let section_id = s(section, "id");
        let knowledge_links: Vec<String> = items_by_section
            .get(&section_id)
            .into_iter()
            .flatten()
            .map(|it| bullet_link(&rel_path, &knowledge_doc_path(it), it["subject"].as_str().unwrap_or("")))
            .collect();
        let section_text = read_section_text(bundle_dir, section);
        let heading = s(section, "heading");
        let mut metadata = okf_metadata(
            "SOP Section",
            &heading,
            &format!("Normalized section from {}.", s(&source, "title")),
            &relative_link(&rel_path, &source_rel),
            &["section", "normalized"],
            Some(vec![okf_source_ref(&rel_path, &source)]),
        );
        let ordinal = match section.get("ordinal").filter(|v| is_truthy(v)) {
            Some(v) => crate::okf_meta::json_to_yaml(v),
            None => match ordinal_from_section_id(&section_id) {
                Some(o) => YamlValue::Raw(o.to_string()),
                None => YamlValue::Raw("null".to_string()),
            },
        };
        let mut sopkb = OrderedMap::new();
        sopkb.insert("section_id", YamlValue::Scalar(section_id.clone()));
        sopkb.insert("source_id", YamlValue::Scalar(source_id.clone()));
        sopkb.insert("source_version_id", opt_scalar(section, "source_version_id"));
        sopkb.insert("ordinal", ordinal);
        sopkb.insert("normalized_path", opt_scalar(section, "normalized_path"));
        metadata.insert("sopkb", YamlValue::Mapping(sopkb));
        if source.get("stale_after").is_some_and(is_truthy) {
            metadata.insert("stale_after", YamlValue::Scalar(s(&source, "stale_after")));
        }
        let body = [
            format!("# {heading}"),
            String::new(),
            format!("Source: [{}]({})", s(&source, "title"), relative_link(&rel_path, &source_rel)),
            String::new(),
            "## Knowledge Pieces".to_string(),
            String::new(),
            or_placeholder(knowledge_links, "- No knowledge pieces mined from this section.").join("\n"),
            String::new(),
            "## Source Excerpt".to_string(),
            String::new(),
            section_text,
            String::new(),
        ]
        .join("\n");
        write_okf_doc(export_dir, &rel_path, &metadata, &body)?;
        entries.push(DirEntry { title: heading, path: rel_path, description: s(&source, "title") });
    }
    Ok(entries)
}

/// `read_section_text` (§3.11 detail). Reads through `read_text_universal_newlines`
/// (raw file text embedded verbatim into a Markdown body -- see the CRLF gotcha in the
/// crate-level docs).
pub fn read_section_text(bundle_dir: &Path, section: &Value) -> String {
    let normalized_path = section.get("normalized_path").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
    let Some(np) = normalized_path else {
        return "_No normalized text available._".to_string();
    };
    let path = bundle_dir.join(np);
    if !path.exists() {
        return "_Normalized text file is missing._".to_string();
    }
    let raw = store::read_text_universal_newlines(&path).unwrap_or_default();
    let mut text = raw.trim().to_string();
    let heading = section["heading"].as_str().unwrap_or("").trim().to_string();
    if !heading.is_empty() && text.contains(&heading) {
        let mut capture: Vec<&str> = Vec::new();
        let mut capturing = false;
        for line in text.lines() {
            let stripped = line.trim();
            if stripped.starts_with('#') {
                if capturing {
                    break;
                }
                if stripped.trim_start_matches('#').trim() == heading {
                    capturing = true;
                }
            }
            if capturing {
                capture.push(line);
            }
        }
        let joined = capture.join("\n").trim().to_string();
        if !joined.is_empty() {
            text = joined;
        }
    }
    if text.is_empty() {
        "_No normalized text available._".to_string()
    } else {
        text
    }
}

/// `write_knowledge_docs` (§3.12). Iterates `sorted(items, key=id)`.
pub fn write_knowledge_docs(export_dir: &Path, items: &[Value], source_by_id: &HashMap<String, Value>, latest_review_by_item: &HashMap<String, Value>) -> Result<Vec<DirEntry>> {
    let mut entries = Vec::new();
    for item in sorted_by_id(items) {
        let item_id = s(item, "id");
        let source_id = s(item, "source_id");
        let source = source_or_fallback(source_by_id, &source_id);
        let rel_path = knowledge_doc_path(item);
        let source_ref = okf_source_ref(&rel_path, &source);
        let subject = s(item, "subject");
        let predicate = s(item, "predicate");
        let object = s(item, "object");
        let review_status = s(item, "review_status");
        let mut metadata = okf_metadata(
            "SOP Knowledge Piece",
            &subject,
            &object,
            &relative_link(&rel_path, &section_doc_path(item)),
            &["knowledge", &predicate, &review_status],
            Some(vec![source_ref.clone()]),
        );
        metadata.insert("status", YamlValue::Scalar(okf_status_for_review(&review_status).to_string()));
        if let Some(review) = latest_review_by_item.get(&item_id) {
            if review_status == "approved" || review_status == "edited" {
                let actor = review.get("reviewer").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).unwrap_or("human:reviewer");
                // G1 [PRESERVE -- not decided as FIX, see DEVIATIONS.md]: `review.get("created_at")`
                // reads a field `ReviewEvent` never has (it's `timestamp`), so `date` is
                // always null. Reproduced verbatim.
                let mut verified_entry = OrderedMap::new();
                verified_entry.insert("actor", YamlValue::Scalar(actor.to_string()));
                verified_entry.insert("date", YamlValue::Raw("null".to_string()));
                metadata.insert("verified", YamlValue::Sequence(vec![YamlValue::Mapping(verified_entry)]));
            }
        }
        let decision_rule_paths: Vec<YamlValue> =
            decision_rules_with_raw(item).iter().map(|(rule, _)| YamlValue::Scalar(relative_link(&rel_path, &rule_doc_path(rule)))).collect();
        let mut sopkb = OrderedMap::new();
        sopkb.insert("knowledge_item_id", YamlValue::Scalar(item_id.clone()));
        sopkb.insert("source_id", YamlValue::Scalar(source_id.clone()));
        sopkb.insert("source_version_id", opt_scalar(item, "source_version_id"));
        sopkb.insert("section_id", YamlValue::Scalar(s(item, "section_id")));
        sopkb.insert("review_status", YamlValue::Scalar(review_status.clone()));
        // Sits immediately after `review_status`, and is deliberately separate from it:
        // an item can be `approved` AND `superseded` at once.
        sopkb.insert("lifecycle_status", YamlValue::Scalar(lifecycle_status_of(item)));
        sopkb.insert("confidence", crate::okf_meta::json_to_yaml(&item["confidence"]));
        sopkb.insert("span_status", YamlValue::Scalar(s(item, "span_status")));
        sopkb.insert("evidence", YamlValue::Scalar(relative_link(&rel_path, &evidence_doc_path(item))));
        sopkb.insert("knowledge_relation", YamlValue::Scalar(relative_link(&rel_path, &relation_doc_path(item))));
        sopkb.insert("decision_rules", YamlValue::Sequence(decision_rule_paths));
        let mut structured = OrderedMap::new();
        structured.insert("subject", YamlValue::Scalar(subject.clone()));
        structured.insert("predicate", YamlValue::Scalar(predicate.clone()));
        structured.insert("object", YamlValue::Scalar(object.clone()));
        sopkb.insert("structured_statement", YamlValue::Mapping(structured));
        metadata.insert("sopkb", YamlValue::Mapping(sopkb));
        if source.get("stale_after").is_some_and(is_truthy) {
            metadata.insert("stale_after", YamlValue::Scalar(s(&source, "stale_after")));
        }

        let subject_concept = concept_for_label(&subject);
        let rule_lines: Vec<String> =
            decision_rules_with_raw(item).iter().map(|(rule, _)| format!("- [{}]({})", rule["title"].as_str().unwrap_or(""), relative_link(&rel_path, &rule_doc_path(rule)))).collect();
        let evidence_id = evidence_id_for_item(item);
        let relation_id = relation_id_for_item(item);
        let source_text = s(item, "source_text");
        let section_id = s(item, "section_id");
        let mut body_lines = vec![
            format!("# {subject}"),
            String::new(),
            "## Structured Statement".to_string(),
            String::new(),
            "| Field | Value |".to_string(),
            "| --- | --- |".to_string(),
            format!("| Subject | [{subject}]({}) |", relative_link(&rel_path, &concept_doc_path(&subject_concept.id))),
            format!("| Predicate | `{predicate}` |"),
            format!("| Object | {object} |"),
            String::new(),
            "## Evidence".to_string(),
            String::new(),
            format!("- [{evidence_id}]({})", relative_link(&rel_path, &evidence_doc_path(item))),
            String::new(),
            "## Relations".to_string(),
            String::new(),
            format!("- [{relation_id}]({})", relative_link(&rel_path, &relation_doc_path(item))),
            String::new(),
            "## Decision Rules".to_string(),
            String::new(),
        ];
        body_lines.extend(rule_lines);
        body_lines.push(String::new());
        body_lines.push("## Source Context".to_string());
        body_lines.push(String::new());
        let source_ref_id = source_ref.as_mapping().and_then(|m| m.get("id")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        body_lines.push(format!("{source_text} [^{source_ref_id}]"));
        body_lines.push(String::new());
        body_lines.push(format!("[^{source_ref_id}]: {} section `{section_id}`.", s(&source, "title")));
        body_lines.push(String::new());
        let body = body_lines.join("\n");
        write_okf_doc(export_dir, &rel_path, &metadata, &body)?;
        entries.push(DirEntry { title: subject, path: rel_path, description: review_status });
    }
    Ok(entries)
}

/// `write_rule_docs` (§3.13). Iterates `sorted(items, key=id)`, then each rule.
pub fn write_rule_docs(export_dir: &Path, items: &[Value], source_by_id: &HashMap<String, Value>) -> Result<Vec<DirEntry>> {
    let mut entries = Vec::new();
    for item in sorted_by_id(items) {
        let source_id = s(item, "source_id");
        let source = source_or_fallback(source_by_id, &source_id);
        let review_status = s(item, "review_status");
        for (rule, raw) in decision_rules_with_raw(item) {
            let rel_path = rule_doc_path(&rule);
            let title = s(&rule, "title");
            let source_ref = okf_source_ref(&rel_path, &source);
            let mut metadata = okf_metadata(
                "SOP Decision Rule",
                &title,
                &rule_description(&rule),
                &relative_link(&rel_path, &knowledge_doc_path(item)),
                &["decision-rule", &review_status],
                Some(vec![source_ref]),
            );
            metadata.insert("status", YamlValue::Scalar(okf_status_for_review(&review_status).to_string()));
            let mut sopkb = OrderedMap::new();
            sopkb.insert("rule", decision_rule_to_yaml(&rule, raw.as_ref()));
            sopkb.insert("knowledge_piece", YamlValue::Scalar(relative_link(&rel_path, &knowledge_doc_path(item))));
            sopkb.insert("knowledge_relation", YamlValue::Scalar(relative_link(&rel_path, &relation_doc_path(item))));
            sopkb.insert("evidence", YamlValue::Scalar(relative_link(&rel_path, &evidence_doc_path(item))));
            metadata.insert("sopkb", YamlValue::Mapping(sopkb));
            let item_id = s(item, "id");
            let body = [
                format!("# {title}"),
                String::new(),
                "## Rule".to_string(),
                String::new(),
                rule_body_lines(&rule).join("\n"),
                String::new(),
                "## Connected Knowledge".to_string(),
                String::new(),
                format!("- Knowledge piece: [{item_id}]({})", relative_link(&rel_path, &knowledge_doc_path(item))),
                format!("- Knowledge relation: [{}]({})", relation_id_for_item(item), relative_link(&rel_path, &relation_doc_path(item))),
                format!("- Evidence: [{}]({})", evidence_id_for_item(item), relative_link(&rel_path, &evidence_doc_path(item))),
                String::new(),
            ]
            .join("\n");
            write_okf_doc(export_dir, &rel_path, &metadata, &body)?;
            entries.push(DirEntry { title, path: rel_path, description: review_status.clone() });
        }
    }
    Ok(entries)
}

fn rule_description(rule: &Value) -> String {
    if let Some(condition) = rule.get("condition").filter(|c| !c.is_null()) {
        let label = condition["label"].as_str().unwrap_or("").to_lowercase();
        let obligation_label = rule["obligation"]["label"].as_str().unwrap_or("").to_lowercase();
        format!("When {label}, {obligation_label}.")
    } else {
        rule["obligation"]["label"].as_str().unwrap_or("").to_string()
    }
}

fn rule_body_lines(rule: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(condition) = rule.get("condition").filter(|c| !c.is_null()) {
        let fact = condition["fact"].as_str().unwrap_or("");
        let operator = condition["operator"].as_str().unwrap_or("").replace('_', " ");
        lines.push(format!("- Condition: `{fact}` {operator}"));
    } else {
        lines.push("- Condition: always applies".to_string());
    }
    lines.push(format!("- Obligation: `{}`", rule["obligation"]["fact"].as_str().unwrap_or("")));
    if let Some(otherwise) = rule.get("otherwise").filter(|o| !o.is_null()) {
        lines.push(format!("- Otherwise: {}", otherwise["label"].as_str().unwrap_or("")));
    }
    lines.push(format!("- Review status: `{}`", rule["review_status"].as_str().unwrap_or("")));
    lines
}

/// `write_relation_docs` (§3.14). Iterates `sorted(items, key=id)`. No `status`
/// override -- relation docs are always `"stable"`, even for rejected knowledge.
pub fn write_relation_docs(export_dir: &Path, items: &[Value], source_by_id: &HashMap<String, Value>) -> Result<Vec<DirEntry>> {
    let mut entries = Vec::new();
    for item in sorted_by_id(items) {
        let source_id = s(item, "source_id");
        let source = source_or_fallback(source_by_id, &source_id);
        let rel_path = relation_doc_path(item);
        let relation = sopkb_derive::relations::knowledge_relation_for_item(item);
        let relation_id = relation["id"].as_str().unwrap_or("").to_string();
        let subject_label = relation["subject"]["label"].as_str().unwrap_or("").to_string();
        let subject_id = relation["subject"]["id"].as_str().unwrap_or("").to_string();
        let predicate_text = relation["predicate"]["text"].as_str().unwrap_or("").to_string();
        let object_label = relation["object"]["label"].as_str().unwrap_or("").to_string();
        let object_text = relation["object"]["text"].as_str().unwrap_or("").to_string();
        let predicate = s(item, "predicate");
        let source_ref = okf_source_ref(&rel_path, &source);
        let metadata_base = okf_metadata(
            "SOP Knowledge Relation",
            &relation_id,
            &format!("{subject_label} {predicate_text} {object_label}."),
            &relative_link(&rel_path, &knowledge_doc_path(item)),
            &["relation", "rdf-compatible", &predicate],
            Some(vec![source_ref]),
        );
        let mut metadata = metadata_base;
        let mut sopkb = OrderedMap::new();
        sopkb.insert("relation", relation_to_yaml(item));
        metadata.insert("sopkb", YamlValue::Mapping(sopkb));
        let item_id = s(item, "id");
        let rule_lines: Vec<String> = decision_rules_with_raw(item)
            .iter()
            .map(|(rule, _)| format!("- Decision rule: [{}]({})", rule["title"].as_str().unwrap_or(""), relative_link(&rel_path, &rule_doc_path(rule))))
            .collect();
        let mut body_lines = vec![
            format!("# {relation_id}"),
            String::new(),
            "## Assertion".to_string(),
            String::new(),
            format!("- Subject: [{subject_label}]({})", relative_link(&rel_path, &concept_doc_path(&subject_id))),
            format!("- Predicate: `{predicate_text}`"),
            format!("- Object: {object_text}"),
            String::new(),
            "## Connected Knowledge".to_string(),
            String::new(),
            format!("- Knowledge piece: [{item_id}]({})", relative_link(&rel_path, &knowledge_doc_path(item))),
            format!("- Evidence: [{}]({})", evidence_id_for_item(item), relative_link(&rel_path, &evidence_doc_path(item))),
        ];
        body_lines.extend(rule_lines);
        body_lines.push(String::new());
        let body = body_lines.join("\n");
        write_okf_doc(export_dir, &rel_path, &metadata, &body)?;
        entries.push(DirEntry { title: relation_id, path: rel_path, description: predicate });
    }
    Ok(entries)
}

/// `write_evidence_docs` (§3.15). No `status` override -- always `"stable"`.
pub fn write_evidence_docs(export_dir: &Path, items: &[Value], source_by_id: &HashMap<String, Value>) -> Result<Vec<DirEntry>> {
    let mut entries = Vec::new();
    for item in sorted_by_id(items) {
        let source_id = s(item, "source_id");
        let source = source_or_fallback(source_by_id, &source_id);
        let rel_path = evidence_doc_path(item);
        let title = evidence_id_for_item(item);
        let subject = s(item, "subject");
        let span_status = s(item, "span_status");
        let source_ref = okf_source_ref(&rel_path, &source);
        let mut metadata = okf_metadata(
            "SOP Evidence",
            &title,
            &format!("Evidence span supporting {subject}."),
            &relative_link(&rel_path, &source_doc_path(&source)),
            &["evidence", &span_status],
            Some(vec![source_ref]),
        );
        let mut sopkb = OrderedMap::new();
        sopkb.insert("knowledge_item_id", YamlValue::Scalar(s(item, "id")));
        sopkb.insert("source_id", YamlValue::Scalar(source_id.clone()));
        sopkb.insert("source_version_id", opt_scalar(item, "source_version_id"));
        sopkb.insert("section_id", YamlValue::Scalar(s(item, "section_id")));
        sopkb.insert("span_status", YamlValue::Scalar(span_status.clone()));
        sopkb.insert("start_pos", crate::okf_meta::json_to_yaml(&item["start_pos"]));
        sopkb.insert("end_pos", crate::okf_meta::json_to_yaml(&item["end_pos"]));
        metadata.insert("sopkb", YamlValue::Mapping(sopkb));
        let source_text = s(item, "source_text");
        let item_id = s(item, "id");
        let body = [
            format!("# {title}"),
            String::new(),
            "## Evidence Span".to_string(),
            String::new(),
            format!("> {source_text}"),
            String::new(),
            "## Supports".to_string(),
            String::new(),
            format!("- [{item_id}]({})", relative_link(&rel_path, &knowledge_doc_path(item))),
            format!("- [{}]({})", relation_id_for_item(item), relative_link(&rel_path, &relation_doc_path(item))),
            String::new(),
        ]
        .join("\n");
        write_okf_doc(export_dir, &rel_path, &metadata, &body)?;
        entries.push(DirEntry { title, path: rel_path, description: subject });
    }
    Ok(entries)
}

struct ConceptAgg {
    id: String,
    label: String,
    knowledge_items: Vec<Value>,
    relations: Vec<Value>,
    sections: Vec<DirEntry>,
}

/// `write_concept_docs` (§3.16). Concepts are derived ONLY from item subjects here
/// (distinct from the section-heading concept population the graph export creates --
/// G20). `section_entry_by_slug` keeps the LAST section with a colliding heading slug.
pub fn write_concept_docs(export_dir: &Path, items: &[Value], section_entries: &[DirEntry]) -> Result<Vec<DirEntry>> {
    let mut order: Vec<String> = Vec::new();
    let mut concepts: HashMap<String, ConceptAgg> = HashMap::new();
    for item in items {
        let subject = item["subject"].as_str().unwrap_or("");
        let concept = concept_for_label(subject);
        let agg = concepts.entry(concept.id.clone()).or_insert_with(|| {
            order.push(concept.id.clone());
            ConceptAgg { id: concept.id.clone(), label: concept.label.clone(), knowledge_items: Vec::new(), relations: Vec::new(), sections: Vec::new() }
        });
        agg.knowledge_items.push(item.clone());
        agg.relations.push(item.clone());
    }
    let mut section_entry_by_slug: HashMap<String, &DirEntry> = HashMap::new();
    for entry in section_entries {
        section_entry_by_slug.insert(sopkb_core::ids::slugify(&entry.title), entry);
    }
    for id in &order {
        let agg = concepts.get_mut(id).unwrap();
        if let Some(entry) = section_entry_by_slug.get(&sopkb_core::ids::slugify(&agg.label)) {
            agg.sections.push((*entry).clone());
        }
    }

    let mut ids: Vec<&String> = order.iter().collect();
    ids.sort();
    let mut entries = Vec::new();
    for id in ids {
        let concept = &concepts[id];
        let rel_path = concept_doc_path(&concept.id);
        let knowledge_links: Vec<String> = concept.knowledge_items.iter().map(|it| bullet_link(&rel_path, &knowledge_doc_path(it), it["object"].as_str().unwrap_or(""))).collect();
        let relation_links: Vec<String> = concept.relations.iter().map(|it| bullet_link(&rel_path, &relation_doc_path(it), &relation_id_for_item(it))).collect();
        let section_links: Vec<String> = concept.sections.iter().map(|entry| bullet_link(&rel_path, &entry.path, &entry.description)).collect();
        let mut metadata = okf_metadata(
            "SOP Concept",
            &concept.label,
            &format!("Concept connected to {} knowledge piece(s).", concept.knowledge_items.len()),
            &rel_path,
            &["concept"],
            None,
        );
        let mut sopkb = OrderedMap::new();
        sopkb.insert("concept_id", YamlValue::Scalar(concept.id.clone()));
        metadata.insert("sopkb", YamlValue::Mapping(sopkb));
        let body = [
            format!("# {}", concept.label),
            String::new(),
            "## Knowledge Pieces".to_string(),
            String::new(),
            or_placeholder(knowledge_links, "- No knowledge pieces.").join("\n"),
            String::new(),
            "## Knowledge Relations".to_string(),
            String::new(),
            or_placeholder(relation_links, "- No knowledge relations.").join("\n"),
            String::new(),
            "## Source Sections".to_string(),
            String::new(),
            or_placeholder(section_links, "- No normalized sections.").join("\n"),
            String::new(),
        ]
        .join("\n");
        write_okf_doc(export_dir, &rel_path, &metadata, &body)?;
        entries.push(DirEntry { title: concept.label.clone(), path: rel_path, description: "concept".to_string() });
    }
    Ok(entries)
}

/// `write_task_docs` (§3.17): the three fixed `TASK_DEFINITIONS`, in list order.
pub fn write_task_docs(export_dir: &Path) -> Result<Vec<DirEntry>> {
    let mut entries = Vec::new();
    for task in sopkb_derive::constants::TASK_DEFINITIONS {
        let rel_path = format!("tasks/{}.md", task.id);
        let mut metadata = okf_metadata("SOP Agent Task Context", task.title, task.description, &rel_path, &["agent-task", task.id], None);
        let mut sopkb = OrderedMap::new();
        sopkb.insert("task_id", YamlValue::Scalar(task.id.to_string()));
        sopkb.insert("query_terms", YamlValue::Sequence(task.query_terms.iter().map(|t| YamlValue::Scalar((*t).to_string())).collect()));
        sopkb.insert("agent_cli", YamlValue::Scalar(format!("sopkb agent context <bundle_dir> --task {}", task.id)));
        metadata.insert("sopkb", YamlValue::Mapping(sopkb));
        let body = [
            format!("# {}", task.title),
            String::new(),
            task.description.to_string(),
            String::new(),
            "## Agent Use".to_string(),
            String::new(),
            format!("- Retrieve context with `sopkb agent context <bundle_dir> --task {}`.", task.id),
            "- Use returned Knowledge Relations for graph traversal.".to_string(),
            "- Resolve evidence before applying any rule.".to_string(),
            String::new(),
        ]
        .join("\n");
        write_okf_doc(export_dir, &rel_path, &metadata, &body)?;
        entries.push(DirEntry { title: task.title.to_string(), path: rel_path, description: task.id.to_string() });
    }
    Ok(entries)
}

/// `write_reference_docs` (§3.18): a single fixed `references/agent-guide.md`.
pub fn write_reference_docs(export_dir: &Path) -> Result<Vec<DirEntry>> {
    let rel_path = "references/agent-guide.md".to_string();
    let metadata = okf_metadata("SOP Agent Guide", "SOP KB Agent Guide", "How agents should consume this OKF bundle.", &rel_path, &["agent-guide", "reference"], None);
    let body = [
        "# SOP KB Agent Guide".to_string(),
        String::new(),
        "Use this bundle as a read-only task context unless a human review workflow explicitly enables writes.".to_string(),
        String::new(),
        "## Consumption Flow".to_string(),
        String::new(),
        "1. Read [Agent Task Contexts](../tasks/index.md) and choose the closest task.".to_string(),
        "2. Retrieve `agent.context` through CLI or MCP for usable knowledge, evidence, reports, concepts, and Knowledge Relations.".to_string(),
        "3. Follow links from knowledge pieces to evidence before making a claim.".to_string(),
        "4. Treat Knowledge Relations as RDF-compatible assertions connected to the supporting knowledge piece.".to_string(),
        "5. Check freshness and conflict reports before operational use.".to_string(),
        String::new(),
        "## Safety Rules".to_string(),
        String::new(),
        "- Do not use rejected knowledge unless explicitly asked to audit rejected material.".to_string(),
        "- Do not infer approval from generated knowledge. Human review is represented by `verified` metadata.".to_string(),
        "- Preserve unknown OKF frontmatter fields when updating documents.".to_string(),
        String::new(),
    ]
    .join("\n");
    write_okf_doc(export_dir, &rel_path, &metadata, &body)?;
    Ok(vec![DirEntry { title: "SOP KB Agent Guide".to_string(), path: rel_path, description: "agent guide".to_string() }])
}

/// `copy_authored_okf_docs` (§3.19). Only `*.md` is copied (P-X2 sibling behavior);
/// `authored/` is created even when there is nothing to copy, since `reset_okf_documents`
/// already wiped it and something must remain a valid (empty) directory.
pub fn copy_authored_okf_docs(bundle_dir: &Path, export_dir: &Path) -> Result<Vec<DirEntry>> {
    let source_root = store::state_path(bundle_dir, "authored_okf");
    let target_root = export_dir.join("authored");
    std::fs::create_dir_all(&target_root)?;
    let mut entries = Vec::new();
    if !source_root.exists() {
        return Ok(entries);
    }
    let mut md_files: Vec<std::path::PathBuf> = Vec::new();
    collect_md_files(&source_root, &mut md_files)?;
    md_files.sort();
    for src in md_files {
        let relative = src.strip_prefix(&source_root).unwrap();
        let target_path = target_root.join(relative);
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&src, &target_path)?;
        let rel_posix: Vec<&str> = relative.components().map(|c| c.as_os_str().to_str().unwrap_or("")).collect();
        let rel_posix = rel_posix.join("/");
        let title = rel_posix.strip_suffix(".md").unwrap_or(&rel_posix).to_string();
        entries.push(DirEntry { title, path: format!("authored/{rel_posix}"), description: "llm-authored".to_string() });
    }
    Ok(entries)
}

fn collect_md_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_md_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(path);
        }
    }
    Ok(())
}

/// `write_directory_index` (§3.20). No frontmatter. Case-insensitive sort by title
/// (Python's stable sort preserves insertion order among equal-lowercased titles).
pub fn write_directory_index(export_dir: &Path, directory: &str, title: &str, entries: &[DirEntry]) -> Result<()> {
    let mut lines = vec![format!("# {title}"), String::new()];
    if entries.is_empty() {
        lines.push("- No entries.".to_string());
    } else {
        let mut sorted: Vec<&DirEntry> = entries.iter().collect();
        sorted.sort_by_key(|e| e.title.to_lowercase());
        let index_path = format!("{directory}/index.md");
        for entry in sorted {
            let target = relative_link(&index_path, &entry.path);
            lines.push(format!("- [{}]({target}) - {}", entry.title, entry.description));
        }
    }
    lines.push(String::new());
    let path = export_dir.join(directory).join("index.md");
    store::write_text_native(&path, &lines.join("\n"))
}
