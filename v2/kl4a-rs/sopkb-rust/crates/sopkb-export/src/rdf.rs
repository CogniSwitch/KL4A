//! `export_rdf` + `ttl_name`/`escape_ttl`. docs/port/port-mapping-c-review-export.md §3.22.
//!
//! This is the one place Phase 5 deliberately DIFFERS from the raw Python behavior
//! the port-mapping doc pseudocodes (§3.22, G17), because PORT_PLAN.md §6.6's own
//! "Work" line explicitly instructs applying P-G5/P-G6/P-G7/P-G12 for this phase (each
//! marked plain **FIX**, not DECIDE, in §5) -- see DEVIATIONS.md for all four entries.

use crate::bundle_export::export_path_for_manifest;
use sopkb_core::error::Result;
use sopkb_core::ids::slugify;
use sopkb_core::store;
use sopkb_derive::relations::{evidence_id_for_item, knowledge_relation_for_item};
use sopkb_derive::rules::decision_rules_for_item;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

/// `ttl_name(v) = slugify(v).replace("-", "_")`.
pub fn ttl_name(v: &str) -> String {
    slugify(v).replace('-', "_")
}

/// `escape_ttl(v) = v.replace("\\","\\\\").replace('"','\\"').replace("\n"," ")`, PLUS
/// (P-G7 [FIX]) `\r`/`\t` escaping: the original leaves those raw inside a quoted
/// literal, which is technically invalid Turtle for CRLF-derived text (G17). Escaping
/// them cannot collide with the pre-existing `\n`->space substitution (disjoint chars).
pub fn escape_ttl(v: &str) -> String {
    let escaped = v.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ");
    escaped.replace('\r', "\\r").replace('\t', "\\t")
}

/// Deterministic, dependency-free FNV-1a truncated to 8 hex chars -- used only to
/// disambiguate a P-G6 IRI collision (below). Not cryptographic; just needs to be
/// stable across runs of the same build.
fn short_hash(input: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", (hash & 0xFFFF_FFFF) as u32)
}

/// Assigns a stable Turtle local name to a `(label, text)` pair sharing a base name,
/// reusing the same name for byte-identical repeats and appending a short hash suffix
/// for genuine collisions (P-G6 [FIX, Turtle-only -- `object.id` in relation documents
/// is deliberately left untouched, per PORT_PLAN.md §5 P-G6's recommendation]).
struct NameRegistry {
    seen: HashMap<String, Vec<(String, String, String)>>,
}

impl NameRegistry {
    fn new() -> Self {
        Self { seen: HashMap::new() }
    }

    /// Returns `(assigned_name, is_first_occurrence)`.
    fn assign(&mut self, base_name: &str, label: &str, text: &str) -> (String, bool) {
        let list = self.seen.entry(base_name.to_string()).or_default();
        for (l, t, name) in list.iter() {
            if l == label && t == text {
                return (name.clone(), false);
            }
        }
        let name = if list.is_empty() { base_name.to_string() } else { format!("{base_name}_{}", short_hash(&format!("{label}\u{0}{text}"))) };
        list.push((label.to_string(), text.to_string(), name.clone()));
        (name, true)
    }
}

/// `export_rdf(bundle_dir, export_dir) -> dict` (§3.22). Reads `items.json` only
/// (never reviews, sections, or inventory); items.json order, no sorting.
///
/// P-G5 [FIX]: `Concept`/`RelationObject` blocks are emitted at most once per distinct
/// `(name, label[, text])`, instead of once per item (the original re-emits identical
/// blocks for every item sharing a subject/object, which is valid-but-redundant RDF).
/// P-G12 [FIX]: a decision rule whose `obligation.fact` is absent (legal for an
/// authored rule with no `obligation`) is skipped instead of crashing the whole
/// export on a direct-index `KeyError`.
pub fn export_rdf(bundle_dir: &Path, export_dir: &Path) -> Result<Value> {
    let items: Vec<Value> = store::read_state_json(bundle_dir, "items.json", json!([]))?.as_array().cloned().unwrap_or_default();
    let mut lines: Vec<String> = vec![
        "@prefix sopkb: <https://example.org/sopkb/> .".to_string(),
        "@prefix dcterms: <http://purl.org/dc/terms/> .".to_string(),
        "@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .".to_string(),
        String::new(),
    ];

    let mut concepts_seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut objects = NameRegistry::new();

    for item in &items {
        let relation = knowledge_relation_for_item(item);
        let relation_name = ttl_name(relation["id"].as_str().unwrap_or(""));
        let subject_id = relation["subject"]["id"].as_str().unwrap_or("");
        let subject_name = ttl_name(subject_id);
        let subject_label = relation["subject"]["label"].as_str().unwrap_or("");
        let predicate_name = ttl_name(relation["predicate"]["id"].as_str().unwrap_or(""));
        let object_id = relation["object"]["id"].as_str().unwrap_or("");
        let object_base_name = ttl_name(object_id);
        let object_label = relation["object"]["label"].as_str().unwrap_or("");
        let object_text = relation["object"]["text"].as_str().unwrap_or("");
        let (object_name, object_is_new) = objects.assign(&object_base_name, object_label, object_text);
        let item_id = item["id"].as_str().unwrap_or("");
        let item_id_ttl = ttl_name(item_id);
        let evidence_ttl = ttl_name(&evidence_id_for_item(item));

        if concepts_seen.insert(subject_name.clone()) {
            lines.push(format!("sopkb:{subject_name} a sopkb:Concept ;"));
            lines.push(format!("  rdfs:label \"{}\" .", escape_ttl(subject_label)));
            lines.push(String::new());
        }
        if object_is_new {
            lines.push(format!("sopkb:{object_name} a sopkb:RelationObject ;"));
            lines.push(format!("  rdfs:label \"{}\" ;", escape_ttl(object_label)));
            lines.push(format!("  sopkb:objectText \"{}\" .", escape_ttl(object_text)));
            lines.push(String::new());
        }

        lines.push(format!("sopkb:{relation_name} a sopkb:KnowledgeRelation ;"));
        lines.push(format!("  sopkb:subject sopkb:{subject_name} ;"));
        lines.push(format!("  sopkb:predicate sopkb:{predicate_name} ;"));
        lines.push(format!("  sopkb:object sopkb:{object_name} ;"));
        lines.push(format!("  sopkb:knowledgePiece sopkb:{item_id_ttl} ;"));
        lines.push(format!("  sopkb:evidence sopkb:{evidence_ttl} ;"));
        lines.push(format!("  dcterms:source \"{}\" ;", escape_ttl(item["source_id"].as_str().unwrap_or(""))));
        lines.push(format!("  sopkb:reviewStatus \"{}\" .", escape_ttl(item["review_status"].as_str().unwrap_or(""))));
        lines.push(String::new());
        lines.push(format!("sopkb:{subject_name} sopkb:{predicate_name} sopkb:{object_name} ."));
        lines.push(String::new());

        for rule in decision_rules_for_item(item) {
            let Some(obligation_fact) = rule.get("obligation").and_then(|o| o.get("fact")).and_then(|f| f.as_str()) else {
                continue; // P-G12 [FIX]: skip a rule with no obligation.fact rather than crash.
            };
            let condition = rule.get("condition").cloned().unwrap_or(Value::Null);
            let otherwise = rule.get("otherwise").cloned().unwrap_or(Value::Null);
            let condition_fact = condition.get("fact").and_then(|f| f.as_str()).unwrap_or("always");
            let action_required = otherwise.get("action_required").and_then(|b| b.as_bool()).unwrap_or(true);
            lines.push(format!("sopkb:{} a sopkb:DecisionRule ;", ttl_name(rule["id"].as_str().unwrap_or(""))));
            lines.push(format!("  rdfs:label \"{}\" ;", escape_ttl(rule["title"].as_str().unwrap_or(""))));
            lines.push(format!("  sopkb:knowledgePiece sopkb:{item_id_ttl} ;"));
            lines.push(format!("  sopkb:knowledgeRelation sopkb:{relation_name} ;"));
            lines.push(format!("  sopkb:evidence sopkb:{evidence_ttl} ;"));
            lines.push(format!("  sopkb:conditionFact \"{}\" ;", escape_ttl(condition_fact)));
            lines.push(format!("  sopkb:obligationFact \"{}\" ;", escape_ttl(obligation_fact)));
            lines.push(format!("  sopkb:otherwiseActionRequired \"{}\" .", if action_required { "true" } else { "false" }));
            lines.push(String::new());
        }
    }

    let path = export_dir.join("graph").join("triples.ttl");
    store::write_text_native(&path, &lines.join("\n"))?;
    Ok(json!({"type": "rdf", "path": export_path_for_manifest(bundle_dir, &path)}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_ttl_escapes_backslash_quote_newline_cr_tab() {
        assert_eq!(escape_ttl("a\\b\"c\nd\re\tf"), "a\\\\b\\\"c d\\re\\tf");
    }

    #[test]
    fn ttl_name_replaces_dashes_with_underscores() {
        assert_eq!(ttl_name("kr-ki-src-000001"), "kr_ki_src_000001");
    }

    #[test]
    fn name_registry_reuses_identical_reassigns_distinct() {
        let mut reg = NameRegistry::new();
        let (n1, new1) = reg.assign("object_x", "L", "T");
        assert!(new1);
        assert_eq!(n1, "object_x");
        let (n2, new2) = reg.assign("object_x", "L", "T");
        assert!(!new2);
        assert_eq!(n2, "object_x");
        let (n3, new3) = reg.assign("object_x", "L2", "T2");
        assert!(new3);
        assert_ne!(n3, "object_x");
        assert!(n3.starts_with("object_x_"));
    }
}
