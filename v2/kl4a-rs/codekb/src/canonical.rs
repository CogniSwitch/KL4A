//! Port of `kl4a/codekb/canonical.py`.
//!
//! Reduces a bundle's raw mined state (inventory/symbols/relations/knowledge)
//! into one canonical, cross-language-comparable shape. Ported symbol-for-
//! symbol from the Python source (verified via `tools-code` MCP
//! `code_symbols_get`).
//!
//! Depends on `crate::state` (port of `kl4a/codekb/state.py` —
//! `STATE_DIR`, `read_json`, `write_json`), which is **out of scope for this
//! batch** and not yet present in this crate.

use std::path::Path;

use anyhow::Result;
use serde_json::{json, Map, Value};

use crate::bundle_store::is_falsy;
use crate::state::{read_json, write_json, STATE_DIR};

/// Port of `kl4a.codekb.canonical.canonicalize_code_bundle` (`canonical.py:9-68`).
///
/// ```python
/// def canonicalize_code_bundle(bundle_dir: Path) -> dict[str, Any]:
///     inventory = read_json(bundle_dir / STATE_DIR / "code_inventory.json",
///                            {"repository": {}, "sources": []})
///     symbols_state = read_json(bundle_dir / STATE_DIR / "code_symbols.json",
///                                {"modules": [], "symbols": [], "evidence": []})
///     relations_state = read_json(bundle_dir / STATE_DIR / "code_relations.json",
///                                  {"relations": []})
///     knowledge_state = read_json(bundle_dir / STATE_DIR / "code_knowledge.json",
///                                  {"items": []})
///
///     artifacts = [ ... one dict per module ... ]
///     artifacts.extend( ... one dict per symbol ... )
///     canonical = {
///         "bundle_id": bundle_dir.name,
///         "repository": inventory.get("repository", {}),
///         "artifacts": sorted(artifacts, key=lambda item: (item["artifact_type"], item["qualified_name"])),
///         "relations": [canonical_relation(r) for r in relations_state.get("relations", [])],
///         "rules": [canonical_rule(item) for item in knowledge_state.get("items", [])],
///         "state_contracts": state_contracts(symbols_state),
///         "workflows": workflows(relations_state),
///     }
///     write_json(bundle_dir / STATE_DIR / "code_canonical.json", canonical)
///     return canonical
/// ```
pub fn canonicalize_code_bundle(bundle_dir: &Path) -> Result<Value> {
    let inventory = read_json(
        &bundle_dir.join(STATE_DIR).join("code_inventory.json"),
        json!({"repository": {}, "sources": []}),
    );
    let symbols_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_symbols.json"),
        json!({"modules": [], "symbols": [], "evidence": []}),
    );
    let relations_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_relations.json"),
        json!({"relations": []}),
    );
    let knowledge_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_knowledge.json"),
        json!({"items": []}),
    );

    let mut artifacts: Vec<Value> = Vec::new();
    for module in list_field(&symbols_state, "modules") {
        artifacts.push(json!({
            "id": module.get("id"),
            "artifact_type": "module",
            "kind": "module",
            "name": module.get("title"),
            "qualified_name": module.get("qualified_name"),
            "language": module.get("language"),
            "source_id": module.get("source_id"),
            "evidence": Value::Array(vec![]),
        }));
    }
    for symbol in list_field(&symbols_state, "symbols") {
        let evidence_id = symbol.get("evidence_id").cloned().unwrap_or(Value::Null);
        let evidence = if evidence_id.is_null() {
            Value::Array(vec![])
        } else {
            Value::Array(vec![evidence_id])
        };
        artifacts.push(json!({
            "id": symbol.get("id"),
            "artifact_type": "symbol",
            "kind": symbol.get("kind"),
            "name": symbol.get("title"),
            "qualified_name": symbol.get("qualified_name"),
            "language": symbol.get("language"),
            "source_id": symbol.get("source_id"),
            "evidence": evidence,
        }));
    }
    artifacts.sort_by(|a, b| {
        let key = |item: &Value| {
            (
                item.get("artifact_type").and_then(Value::as_str).unwrap_or("").to_string(),
                item.get("qualified_name").and_then(Value::as_str).unwrap_or("").to_string(),
            )
        };
        key(a).cmp(&key(b))
    });

    let relations: Vec<Value> = list_field(&relations_state, "relations")
        .iter()
        .map(|r| canonical_relation(r))
        .collect();
    let rules: Vec<Value> = list_field(&knowledge_state, "items")
        .iter()
        .map(|item| canonical_rule(item))
        .collect();

    let canonical = json!({
        "bundle_id": bundle_dir.file_name().map(|n| n.to_string_lossy().to_string()),
        "repository": inventory.get("repository").cloned().unwrap_or(json!({})),
        "artifacts": artifacts,
        "relations": relations,
        "rules": rules,
        "state_contracts": state_contracts(&symbols_state),
        "workflows": workflows(&relations_state),
    });
    write_json(&bundle_dir.join(STATE_DIR).join("code_canonical.json"), &canonical)?;
    Ok(canonical)
}

/// Port of `kl4a.codekb.canonical.emit_canonical_artifacts` (`canonical.py:71-88`).
///
/// ```python
/// def emit_canonical_artifacts(bundle_dir: Path) -> dict[str, Any]:
///     canonical = canonicalize_code_bundle(bundle_dir)
///     symbols = [
///         {**artifact, "canonical_name": canonical_name(artifact.get("name", "")),
///          "canonical_kind": canonical_kind(artifact.get("kind", ""))}
///         for artifact in canonical["artifacts"]
///         if artifact.get("artifact_type") == "symbol"
///     ]
///     summary = {
///         "symbols_by_language": count_by(symbols, "language"),
///         "symbols_by_canonical_kind": count_by(symbols, "canonical_kind"),
///     }
///     state = {**canonical, "symbols": symbols, "summary": summary}
///     write_json(bundle_dir / STATE_DIR / "code_canonical.json", state)
///     return state
/// ```
pub fn emit_canonical_artifacts(bundle_dir: &Path) -> Result<Value> {
    let canonical = canonicalize_code_bundle(bundle_dir)?;
    let artifacts = canonical
        .get("artifacts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let symbols: Vec<Value> = artifacts
        .iter()
        .filter(|artifact| artifact.get("artifact_type").and_then(Value::as_str) == Some("symbol"))
        .map(|artifact| {
            let name = artifact.get("name").and_then(Value::as_str).unwrap_or("");
            let kind = artifact.get("kind").and_then(Value::as_str).unwrap_or("");
            let mut merged = artifact
                .as_object()
                .cloned()
                .unwrap_or_default();
            merged.insert("canonical_name".to_string(), json!(canonical_name(name)));
            merged.insert("canonical_kind".to_string(), json!(canonical_kind(kind)));
            Value::Object(merged)
        })
        .collect();

    let summary = json!({
        "symbols_by_language": count_by(&symbols, "language"),
        "symbols_by_canonical_kind": count_by(&symbols, "canonical_kind"),
    });

    let mut state = canonical.as_object().cloned().unwrap_or_default();
    state.insert("symbols".to_string(), Value::Array(symbols));
    state.insert("summary".to_string(), summary);
    let state = Value::Object(state);

    write_json(&bundle_dir.join(STATE_DIR).join("code_canonical.json"), &state)?;
    Ok(state)
}

/// Port of `kl4a.codekb.canonical.canonical_relation` (`canonical.py:91-101`).
///
/// ```python
/// def canonical_relation(relation: dict[str, Any]) -> dict[str, Any]:
///     metadata = relation.get("relation") or {}
///     return {
///         "id": relation["id"], "subject": relation["subject"],
///         "predicate": relation["predicate"], "object": relation["object"],
///         "resolution_status": metadata.get("resolution_status"),
///         "confidence": metadata.get("confidence"),
///         "evidence": relation.get("evidence", []),
///     }
/// ```
pub fn canonical_relation(relation: &Value) -> Value {
    let empty = json!({});
    let metadata = relation.get("relation").filter(|v| !v.is_null()).unwrap_or(&empty);
    json!({
        "id": relation.get("id"),
        "subject": relation.get("subject"),
        "predicate": relation.get("predicate"),
        "object": relation.get("object"),
        "resolution_status": metadata.get("resolution_status"),
        "confidence": metadata.get("confidence"),
        "evidence": relation.get("evidence").cloned().unwrap_or(Value::Array(vec![])),
    })
}

/// Port of `kl4a.codekb.canonical.canonical_rule` (`canonical.py:104-115`).
///
/// ```python
/// def canonical_rule(item: dict[str, Any]) -> dict[str, Any]:
///     return {
///         "id": item["id"], "title": item["title"], "claim": item["claim"],
///         "tier": item.get("knowledge_tier"), "review_status": item.get("review_status"),
///         "review_required": item.get("review_required"),
///         "symbols": item.get("symbols", []), "relations": item.get("relations", []),
///         "evidence": item.get("evidence", []),
///     }
/// ```
pub fn canonical_rule(item: &Value) -> Value {
    json!({
        "id": item.get("id"),
        "title": item.get("title"),
        "claim": item.get("claim"),
        "tier": item.get("knowledge_tier"),
        "review_status": item.get("review_status"),
        "review_required": item.get("review_required"),
        "symbols": item.get("symbols").cloned().unwrap_or(Value::Array(vec![])),
        "relations": item.get("relations").cloned().unwrap_or(Value::Array(vec![])),
        "evidence": item.get("evidence").cloned().unwrap_or(Value::Array(vec![])),
    })
}

/// Port of `kl4a.codekb.canonical.count_by` (`canonical.py:183-188`).
///
/// ```python
/// def count_by(items: list[dict[str, Any]], key: str) -> dict[str, int]:
///     counts: dict[str, int] = {}
///     for item in items:
///         value = str(item.get(key) or "unknown")
///         counts[value] = counts.get(value, 0) + 1
///     return dict(sorted(counts.items()))
/// ```
///
/// Returns a `Map` (matches `dict[str, int]`) rather than a Rust struct since
/// the set of keys is data-dependent (whatever `language`/`canonical_kind`
/// values are actually present).
pub fn count_by(items: &[Value], key: &str) -> Map<String, Value> {
    let mut counts: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
    for item in items {
        let field = item.get(key);
        let value = if is_falsy(field) {
            "unknown".to_string()
        } else {
            match field.unwrap() {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            }
        };
        *counts.entry(value).or_insert(0) += 1;
    }
    let mut out = Map::new();
    for (key, value) in counts {
        out.insert(key, json!(value));
    }
    out
}

/// Port of `kl4a.codekb.canonical.state_contracts` (`canonical.py:118-134`).
///
/// ```python
/// def state_contracts(symbols_state: dict[str, Any]) -> list[dict[str, Any]]:
///     contracts = []
///     for symbol in symbols_state.get("symbols", []):
///         if symbol.get("kind") in {"record", "data_item", "data", "file"}:
///             contracts.append({
///                 "id": f"state-contract-{symbol['id']}", "symbol_id": symbol["id"],
///                 "name": symbol["title"], "kind": symbol["kind"], "language": symbol["language"],
///                 "evidence": [symbol.get("evidence_id")] if symbol.get("evidence_id") else [],
///             })
///     return sorted(contracts, key=lambda item: item["id"])
/// ```
pub fn state_contracts(symbols_state: &Value) -> Vec<Value> {
    const ELIGIBLE_KINDS: [&str; 4] = ["record", "data_item", "data", "file"];
    let mut contracts: Vec<Value> = list_field(symbols_state, "symbols")
        .into_iter()
        .filter(|symbol| {
            symbol
                .get("kind")
                .and_then(Value::as_str)
                .map(|kind| ELIGIBLE_KINDS.contains(&kind))
                .unwrap_or(false)
        })
        .map(|symbol| {
            let id = symbol.get("id").and_then(Value::as_str).unwrap_or("");
            let evidence_id = symbol.get("evidence_id").cloned().unwrap_or(Value::Null);
            let evidence = if evidence_id.is_null() {
                Value::Array(vec![])
            } else {
                Value::Array(vec![evidence_id])
            };
            json!({
                "id": format!("state-contract-{id}"),
                "symbol_id": symbol.get("id"),
                "name": symbol.get("title"),
                "kind": symbol.get("kind"),
                "language": symbol.get("language"),
                "evidence": evidence,
            })
        })
        .collect();
    contracts.sort_by(|a, b| {
        let id_of = |v: &Value| v.get("id").and_then(Value::as_str).unwrap_or("").to_string();
        id_of(a).cmp(&id_of(b))
    });
    contracts
}

/// Port of `kl4a.codekb.canonical.workflows` (`canonical.py:137-153`).
///
/// ```python
/// def workflows(relations_state: dict[str, Any]) -> list[dict[str, Any]]:
///     workflow_predicates = {"calls", "performs", "jumps_to", "branches_on"}
///     return [
///         {"id": f"workflow-{relation['id']}", "relation_id": relation["id"],
///          "subject": relation["subject"], "action": relation["predicate"],
///          "object": relation["object"],
///          "confidence": (relation.get("relation") or {}).get("confidence"),
///          "resolution_status": (relation.get("relation") or {}).get("resolution_status")}
///         for relation in relations_state.get("relations", [])
///         if relation.get("predicate") in workflow_predicates
///     ]
/// ```
pub fn workflows(relations_state: &Value) -> Vec<Value> {
    const WORKFLOW_PREDICATES: [&str; 4] = ["calls", "performs", "jumps_to", "branches_on"];
    let empty = json!({});
    list_field(relations_state, "relations")
        .into_iter()
        .filter(|relation| {
            relation
                .get("predicate")
                .and_then(Value::as_str)
                .map(|predicate| WORKFLOW_PREDICATES.contains(&predicate))
                .unwrap_or(false)
        })
        .map(|relation| {
            let id = relation.get("id").and_then(Value::as_str).unwrap_or("");
            let metadata = relation.get("relation").filter(|v| !v.is_null()).unwrap_or(&empty);
            json!({
                "id": format!("workflow-{id}"),
                "relation_id": relation.get("id"),
                "subject": relation.get("subject"),
                "action": relation.get("predicate"),
                "object": relation.get("object"),
                "confidence": metadata.get("confidence"),
                "resolution_status": metadata.get("resolution_status"),
            })
        })
        .collect()
}

/// Port of `kl4a.codekb.canonical.canonical_name` (`canonical.py:156-172`).
///
/// ```python
/// def canonical_name(name: str) -> str:
///     words = []
///     current = ""
///     for char in name.replace("-", "_"):
///         if char == "_":
///             if current:
///                 words.append(current.lower())
///                 current = ""
///             continue
///         if char.isupper() and current:
///             words.append(current.lower())
///             current = char
///         else:
///             current += char
///     if current:
///         words.append(current.lower())
///     return " ".join(word for word in words if word)
/// ```
pub fn canonical_name(name: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in name.replace('-', "_").chars() {
        if ch == '_' {
            if !current.is_empty() {
                words.push(current.to_lowercase());
                current = String::new();
            }
            continue;
        }
        if ch.is_uppercase() && !current.is_empty() {
            words.push(current.to_lowercase());
            current = String::from(ch);
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        words.push(current.to_lowercase());
    }
    words.into_iter().filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Port of `kl4a.codekb.canonical.canonical_kind` (`canonical.py:175-180`).
///
/// ```python
/// def canonical_kind(kind: str) -> str:
///     if kind in {"function", "method", "paragraph", "program"}:
///         return "operation"
///     if kind in {"class", "record", "data_item", "data", "file"}:
///         return "type"
///     return kind or "unknown"
/// ```
pub fn canonical_kind(kind: &str) -> String {
    const OPERATION_KINDS: [&str; 4] = ["function", "method", "paragraph", "program"];
    const TYPE_KINDS: [&str; 5] = ["class", "record", "data_item", "data", "file"];
    if OPERATION_KINDS.contains(&kind) {
        "operation".to_string()
    } else if TYPE_KINDS.contains(&kind) {
        "type".to_string()
    } else if kind.is_empty() {
        "unknown".to_string()
    } else {
        kind.to_string()
    }
}

/// Reads `state.get(field, [])` as a `Vec<Value>`, matching every call site's
/// `.get("...", [])` default in the Python original.
fn list_field(state: &Value, field: &str) -> Vec<Value> {
    state
        .get(field)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}
