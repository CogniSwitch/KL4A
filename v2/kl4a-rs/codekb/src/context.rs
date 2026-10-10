//! Port of `kl4a/codekb/context.py` — the Code Knowledge Access Layer (KAL):
//! every `code.*` read-only query surface (`code_repo_describe`,
//! `code_files_search`, `code_symbols_search`, `code_symbol_get`,
//! `code_relations_search`, `code_relation_neighborhood`,
//! `code_tests_for_symbol`, `code_change_impact`, `code_context`) plus their
//! shared payload/filter helpers.
//!
//! Every symbol below is grounded via tools-code MCP `code_symbols_get`
//! against `kl4a.codekb.context.*`; Python excerpts are quoted per function.
//!
//! ## Cross-batch dependencies (NOT part of this batch — report only)
//!
//! - `crate::state::{read_json, STATE_DIR}` — `kl4a/codekb/state.py`, not in
//!   this batch. Grounded signature: `STATE_DIR: &str = ".codekb"`;
//!   `read_json(path: &Path, default: Value) -> anyhow::Result<Value>`
//!   (tolerates a missing/empty/torn/invalid-JSON file by returning
//!   `default` instead of erroring — confirmed via
//!   `code_symbols_get(symbol-kl4a-codekb-state-read-json)`). Two other
//!   already-ported files in this crate (`model.rs`, `lifecycle.rs`) already
//!   depend on this same not-yet-existing module the same way, so this
//!   reference is consistent with the established cross-batch convention,
//!   not a new gap.
//! - `crate::lifecycle::read_review_state` — already ported in this crate
//!   (`lifecycle.rs`), reused as-is.
//!
//! ## Downstream contract (already-ported files depend on these signatures)
//!
//! `agent.rs` and `mcp.rs` (both already in this crate, from other batches)
//! already call `crate::context::{code_context, code_repo_describe,
//! code_files_search, code_symbols_search, code_symbol_get,
//! code_relations_search, code_relation_neighborhood, code_tests_for_symbol,
//! code_change_impact}` assuming specific signatures. Every public function
//! below matches `mcp.rs`'s `call_mcp_tool` call sites exactly (verified by
//! reading `mcp.rs`), since `mcp.rs` is the more precise/consistent of the
//! two existing call sites. **`agent.rs`'s own call site is a mismatch**:
//! `agent.rs` line 59 calls `code_context(bundle_dir, task, query, None)`
//! where `query: &str` (not wrapped in `Some(..)`), but `code_context` below
//! takes `query: Option<&str>` as its third parameter (matching both
//! `mcp.rs`'s usage and Python's own `query: str | None = None`). `agent.rs`
//! is outside this batch's assigned file list, so it was not edited; the fix
//! needed there is `Some(query)` in place of `query` at that call site —
//! flagged here and in the handback report for the coordinator.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{bail, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Map, Value};

use crate::lifecycle::read_review_state;
use crate::state::{read_json, STATE_DIR};

/// Port of `kl4a.codekb.context._CONTEXT_SYMBOL_LIMIT` (`= 12`).
pub const CONTEXT_SYMBOL_LIMIT: usize = 12;
/// Port of `kl4a.codekb.context._CONTEXT_LIST_LIMIT` (`= 10`).
pub const CONTEXT_LIST_LIMIT: usize = 10;

/// Port of `kl4a.codekb.context._QUERY_TERM_RE` (`re.compile("[a-z0-9]+")`).
static QUERY_TERM_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[a-z0-9]+").unwrap());
/// Port of `kl4a.codekb.context._SEPARATOR_RE` (`re.compile(r"[./_\\-]+")`).
static SEPARATOR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[./_\\-]+").unwrap());

/// Port of `kl4a.codekb.context._normalize_separators`.
///
/// ```python
/// def _normalize_separators(value: str) -> str:
///     return _SEPARATOR_RE.sub("-", value.lower())
/// ```
fn normalize_separators(value: &str) -> String {
    SEPARATOR_RE.replace_all(&value.to_lowercase(), "-").into_owned()
}

/// Port of `kl4a.codekb.context.symbol_payload`.
///
/// ```python
/// def symbol_payload(symbol: dict[str, Any]) -> dict[str, Any]:
///     return {
///         "id": symbol["id"], "title": symbol["title"], "kind": symbol["kind"],
///         "qualified_name": symbol["qualified_name"], "language": symbol["language"],
///         "file": symbol["file"], "line_start": symbol["line_start"], "line_end": symbol["line_end"],
///         "signature": symbol.get("signature"), "evidence_id": symbol.get("evidence_id"),
///         "okf_path": symbol.get("okf_path"),
///     }
/// ```
///
/// Python subscripts (`symbol["id"]`, etc.) require these keys, raising
/// `KeyError` if one is absent from otherwise-trusted internal bundle state;
/// mirrored here leniently as `Value::Null` on a missing key instead, matching
/// this crate's established idiom (`agent.rs`, `mcp.rs`) of tolerant
/// `.get(...).unwrap_or(...)` access rather than a hard error for
/// state-shape mismatches.
pub fn symbol_payload(symbol: &Value) -> Value {
    json!({
        "id": symbol.get("id").cloned().unwrap_or(Value::Null),
        "title": symbol.get("title").cloned().unwrap_or(Value::Null),
        "kind": symbol.get("kind").cloned().unwrap_or(Value::Null),
        "qualified_name": symbol.get("qualified_name").cloned().unwrap_or(Value::Null),
        "language": symbol.get("language").cloned().unwrap_or(Value::Null),
        "file": symbol.get("file").cloned().unwrap_or(Value::Null),
        "line_start": symbol.get("line_start").cloned().unwrap_or(Value::Null),
        "line_end": symbol.get("line_end").cloned().unwrap_or(Value::Null),
        "signature": symbol.get("signature").cloned().unwrap_or(Value::Null),
        "evidence_id": symbol.get("evidence_id").cloned().unwrap_or(Value::Null),
        "okf_path": symbol.get("okf_path").cloned().unwrap_or(Value::Null),
    })
}

/// Port of `kl4a.codekb.context.relation_payload`.
///
/// ```python
/// def relation_payload(relation: dict[str, Any]) -> dict[str, Any]:
///     return {
///         "id": relation["id"], "type": relation.get("type", "Code Knowledge Relation"),
///         "subject": relation["subject"], "predicate": relation["predicate"], "object": relation["object"],
///         "subject_kind": relation.get("subject_kind"), "object_kind": relation.get("object_kind"),
///         "resolution_status": (relation.get("relation") or {}).get("resolution_status"),
///         "confidence": (relation.get("relation") or {}).get("confidence"),
///         "evidence": relation.get("evidence", []), "okf_path": relation.get("okf_path"),
///     }
/// ```
pub fn relation_payload(relation: &Value) -> Value {
    let inner = relation.get("relation").and_then(Value::as_object);
    json!({
        "id": relation.get("id").cloned().unwrap_or(Value::Null),
        "type": relation.get("type").cloned().unwrap_or_else(|| Value::String("Code Knowledge Relation".to_string())),
        "subject": relation.get("subject").cloned().unwrap_or(Value::Null),
        "predicate": relation.get("predicate").cloned().unwrap_or(Value::Null),
        "object": relation.get("object").cloned().unwrap_or(Value::Null),
        "subject_kind": relation.get("subject_kind").cloned().unwrap_or(Value::Null),
        "object_kind": relation.get("object_kind").cloned().unwrap_or(Value::Null),
        "resolution_status": inner.and_then(|r| r.get("resolution_status")).cloned().unwrap_or(Value::Null),
        "confidence": inner.and_then(|r| r.get("confidence")).cloned().unwrap_or(Value::Null),
        "evidence": relation.get("evidence").cloned().unwrap_or(Value::Array(vec![])),
        "okf_path": relation.get("okf_path").cloned().unwrap_or(Value::Null),
    })
}

/// Port of `kl4a.codekb.context.item_payload`.
///
/// ```python
/// def item_payload(item: dict[str, Any]) -> dict[str, Any]:
///     return {
///         "id": item["id"], "title": item["title"], "claim": item["claim"],
///         "knowledge_tier": item["knowledge_tier"], "review_status": item["review_status"],
///         "lifecycle_status": item["lifecycle_status"], "review_required": item["review_required"],
///         "evidence": item.get("evidence", []), "relations": item.get("relations", []),
///         "symbols": item.get("symbols", []), "okf_path": item.get("okf_path"),
///     }
/// ```
pub fn item_payload(item: &Value) -> Value {
    json!({
        "id": item.get("id").cloned().unwrap_or(Value::Null),
        "title": item.get("title").cloned().unwrap_or(Value::Null),
        "claim": item.get("claim").cloned().unwrap_or(Value::Null),
        "knowledge_tier": item.get("knowledge_tier").cloned().unwrap_or(Value::Null),
        "review_status": item.get("review_status").cloned().unwrap_or(Value::Null),
        "lifecycle_status": item.get("lifecycle_status").cloned().unwrap_or(Value::Null),
        "review_required": item.get("review_required").cloned().unwrap_or(Value::Null),
        "evidence": item.get("evidence").cloned().unwrap_or(Value::Array(vec![])),
        "relations": item.get("relations").cloned().unwrap_or(Value::Array(vec![])),
        "symbols": item.get("symbols").cloned().unwrap_or(Value::Array(vec![])),
        "okf_path": item.get("okf_path").cloned().unwrap_or(Value::Null),
    })
}

/// Port of `kl4a.codekb.context.item_is_default_usable`.
///
/// ```python
/// def item_is_default_usable(item: dict[str, Any]) -> bool:
///     """Whether a claim may be handed to an agent without being asked for.
///     Callers must pass items that already carry the reviewer's decision --
///     see :func:`reviewed_code_items`."""
///     if item.get("review_status") in {"rejected"}:
///         return False
///     if item.get("lifecycle_status", "active") != "active":
///         return False
///     if int(item.get("knowledge_tier") or 0) >= 4 and item.get("review_required"):
///         return item.get("review_status") == "approved"
///     return True
/// ```
pub fn item_is_default_usable(item: &Value) -> bool {
    if item.get("review_status").and_then(Value::as_str) == Some("rejected") {
        return false;
    }
    let lifecycle_status = item
        .get("lifecycle_status")
        .and_then(Value::as_str)
        .unwrap_or("active");
    if lifecycle_status != "active" {
        return false;
    }
    let tier = item.get("knowledge_tier").and_then(Value::as_i64).unwrap_or(0);
    let review_required = item.get("review_required").and_then(Value::as_bool).unwrap_or(false);
    if tier >= 4 && review_required {
        return item.get("review_status").and_then(Value::as_str) == Some("approved");
    }
    true
}

/// Port of `kl4a.codekb.context.dedupe_payloads`.
///
/// ```python
/// def dedupe_payloads(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
///     return list({item["id"]: item for item in items}.values())
/// ```
///
/// A Python dict comprehension keeps first-seen *key order* but the *value*
/// stored for a repeated key is whichever occurrence came last — mirrored
/// here the same way (order by first occurrence of each id, value from the
/// last occurrence).
pub fn dedupe_payloads(items: Vec<Value>) -> Vec<Value> {
    let mut order: Vec<String> = Vec::new();
    let mut by_id: HashMap<String, Value> = HashMap::new();
    for item in items {
        let id = item.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
        if !by_id.contains_key(&id) {
            order.push(id.clone());
        }
        by_id.insert(id, item);
    }
    order.into_iter().filter_map(|id| by_id.remove(&id)).collect()
}

/// Port of `kl4a.codekb.context.context_warnings`.
///
/// ```python
/// def context_warnings(relations: list[dict[str, Any]], knowledge: list[dict[str, Any]]) -> list[str]:
///     warnings: list[str] = []
///     unresolved = [r for r in relations if (r.get("relation") or {}).get("resolution_status") == "unresolved"]
///     if unresolved:
///         warnings.append(f"{len(unresolved)} unresolved relation(s) excluded from exact semantic certainty")
///     review_required_count = sum(1 for item in knowledge if item.get("review_required"))
///     if review_required_count:
///         warnings.append(f"{review_required_count} review-required claim(s) present")
///     return warnings
/// ```
///
/// Takes the *raw* relation dicts (not yet `relation_payload`-transformed),
/// matching how `code_context` calls this with its own `relations` list.
pub fn context_warnings(relations: &[Value], knowledge: &[Value]) -> Vec<String> {
    let mut warnings = Vec::new();
    let unresolved = relations
        .iter()
        .filter(|r| {
            r.get("relation")
                .and_then(Value::as_object)
                .and_then(|rel| rel.get("resolution_status"))
                .and_then(Value::as_str)
                == Some("unresolved")
        })
        .count();
    if unresolved > 0 {
        warnings.push(format!(
            "{unresolved} unresolved relation(s) excluded from exact semantic certainty"
        ));
    }
    let review_required_count = knowledge
        .iter()
        .filter(|item| item.get("review_required").and_then(Value::as_bool).unwrap_or(false))
        .count();
    if review_required_count > 0 {
        warnings.push(format!("{review_required_count} review-required claim(s) present"));
    }
    warnings
}

/// Port of `kl4a.codekb.context.relations_for_symbols`.
///
/// ```python
/// def relations_for_symbols(bundle_dir: Path, symbol_ids: set[str]) -> list[dict[str, Any]]:
///     relations = read_json(bundle_dir / STATE_DIR / "code_relations.json", {"relations": []}).get("relations", [])
///     matched = [r for r in relations if r.get("subject") in symbol_ids or r.get("object") in symbol_ids]
///     return sorted(matched, key=lambda item: (item["predicate"], item["subject"], item["object"]))
/// ```
pub fn relations_for_symbols(bundle_dir: &Path, symbol_ids: &HashSet<String>) -> Result<Vec<Value>> {
    let state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_relations.json"),
        json!({"relations": []}),
    );
    let relations = state.get("relations").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut matched: Vec<Value> = relations
        .into_iter()
        .filter(|r| {
            let subject = r.get("subject").and_then(Value::as_str).unwrap_or_default();
            let object = r.get("object").and_then(Value::as_str).unwrap_or_default();
            symbol_ids.contains(subject) || symbol_ids.contains(object)
        })
        .collect();
    sort_by_predicate_subject_object(&mut matched);
    Ok(matched)
}

/// Shared sort key used by [`relations_for_symbols`] and
/// [`code_relations_search`]: `(predicate, subject, object)` ascending.
fn sort_by_predicate_subject_object(items: &mut [Value]) {
    items.sort_by(|a, b| {
        let key = |v: &Value| -> (String, String, String) {
            (
                v.get("predicate").and_then(Value::as_str).unwrap_or_default().to_string(),
                v.get("subject").and_then(Value::as_str).unwrap_or_default().to_string(),
                v.get("object").and_then(Value::as_str).unwrap_or_default().to_string(),
            )
        };
        key(a).cmp(&key(b))
    });
}

/// Port of `kl4a.codekb.context.reviewed_code_items`.
///
/// ```python
/// def reviewed_code_items(bundle_dir: Path) -> list[dict[str, Any]]:
///     """Mined claims with the reviewer's decision applied.
///     Decisions are a non-destructive overlay in ``.codekb/code_reviews.json``;
///     the mined claims themselves are never rewritten."""
///     items = read_json(bundle_dir / STATE_DIR / "code_knowledge.json", {"items": []}).get("items", [])
///     decisions = read_review_state(bundle_dir).get("status", {})
///     if not decisions:
///         return items
///     return [
///         {**item, "review_status": decisions[item["id"]]} if item.get("id") in decisions else item
///         for item in items
///     ]
/// ```
pub fn reviewed_code_items(bundle_dir: &Path) -> Result<Vec<Value>> {
    let state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_knowledge.json"),
        json!({"items": []}),
    );
    let items = state.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
    let review_state = read_review_state(bundle_dir);
    let decisions = review_state
        .get("status")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if decisions.is_empty() {
        return Ok(items);
    }
    Ok(items
        .into_iter()
        .map(|item| {
            let id = item.get("id").and_then(Value::as_str).unwrap_or_default();
            match decisions.get(id) {
                Some(status) => {
                    let mut updated = item;
                    if let Value::Object(map) = &mut updated {
                        map.insert("review_status".to_string(), status.clone());
                    }
                    updated
                }
                None => item,
            }
        })
        .collect())
}

/// Port of `kl4a.codekb.context._covers_index`.
///
/// ```python
/// def _covers_index(bundle_dir: Path) -> tuple[dict[str, dict[str, Any]], dict[str, set[str]]]:
///     """Read relations/symbols once and index which test symbols cover which
///     symbol, keyed by the covered symbol's id (`covers`'s object, or
///     `tested_by`'s subject -- both name the *covered* symbol)."""
///     relations = read_json(bundle_dir / STATE_DIR / "code_relations.json", {"relations": []}).get("relations", [])
///     symbols = read_json(bundle_dir / STATE_DIR / "code_symbols.json", {"symbols": []}).get("symbols", [])
///     symbols_by_id = {symbol["id"]: symbol for symbol in symbols}
///     tests_by_symbol_id: dict[str, set[str]] = {}
///     for relation in relations:
///         predicate = relation.get("predicate")
///         if predicate == "covers":
///             tests_by_symbol_id.setdefault(relation.get("object"), set()).add(relation.get("subject"))
///         elif predicate == "tested_by":
///             tests_by_symbol_id.setdefault(relation.get("subject"), set()).add(relation.get("object"))
///     return symbols_by_id, tests_by_symbol_id
/// ```
fn covers_index(bundle_dir: &Path) -> Result<(HashMap<String, Value>, HashMap<String, HashSet<String>>)> {
    let relations_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_relations.json"),
        json!({"relations": []}),
    );
    let symbols_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_symbols.json"),
        json!({"symbols": []}),
    );

    let symbols_by_id: HashMap<String, Value> = symbols_state
        .get("symbols")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(|id| (id.to_string(), s.clone())))
        .collect();

    let mut tests_by_symbol_id: HashMap<String, HashSet<String>> = HashMap::new();
    for relation in relations_state
        .get("relations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let predicate = relation.get("predicate").and_then(Value::as_str).unwrap_or_default();
        let subject = relation.get("subject").and_then(Value::as_str).unwrap_or_default().to_string();
        let object = relation.get("object").and_then(Value::as_str).unwrap_or_default().to_string();
        match predicate {
            "covers" => {
                tests_by_symbol_id.entry(object).or_default().insert(subject);
            }
            "tested_by" => {
                tests_by_symbol_id.entry(subject).or_default().insert(object);
            }
            _ => {}
        }
    }
    Ok((symbols_by_id, tests_by_symbol_id))
}

/// Port of `kl4a.codekb.context._tests_for_symbol_from_index`.
///
/// ```python
/// def _tests_for_symbol_from_index(symbol_id, symbols_by_id, tests_by_symbol_id) -> list[dict[str, Any]]:
///     test_ids = tests_by_symbol_id.get(symbol_id, set())
///     return [symbol_payload(symbols_by_id[test_id]) for test_id in sorted(test_ids) if test_id in symbols_by_id]
/// ```
fn tests_for_symbol_from_index(
    symbol_id: &str,
    symbols_by_id: &HashMap<String, Value>,
    tests_by_symbol_id: &HashMap<String, HashSet<String>>,
) -> Vec<Value> {
    let mut test_ids: Vec<&String> = tests_by_symbol_id
        .get(symbol_id)
        .map(|s| s.iter().collect())
        .unwrap_or_default();
    test_ids.sort();
    test_ids
        .into_iter()
        .filter_map(|tid| symbols_by_id.get(tid))
        .map(symbol_payload)
        .collect()
}

/// Port of `kl4a.codekb.context.code_tests_for_symbol`.
///
/// ```python
/// def code_tests_for_symbol(bundle_dir: Path, symbol_id: str) -> list[dict[str, Any]]:
///     symbols_by_id, tests_by_symbol_id = _covers_index(bundle_dir)
///     return _tests_for_symbol_from_index(symbol_id, symbols_by_id, tests_by_symbol_id)
/// ```
pub fn code_tests_for_symbol(bundle_dir: &Path, symbol_id: &str) -> Result<Value> {
    let (symbols_by_id, tests_by_symbol_id) = covers_index(bundle_dir)?;
    let tests = tests_for_symbol_from_index(symbol_id, &symbols_by_id, &tests_by_symbol_id);
    Ok(Value::Array(tests))
}

/// Port of `kl4a.codekb.context.code_relation_neighborhood`.
///
/// ```python
/// def code_relation_neighborhood(bundle_dir: Path, node_id: str, *, depth: int=1) -> dict[str, Any]:
///     relations = read_json(bundle_dir / STATE_DIR / "code_relations.json", {"relations": []}).get("relations", [])
///     frontier = {node_id}; visited = {node_id}; matched: list[dict[str, Any]] = []
///     for _ in range(max(1, depth)):
///         next_frontier: set[str] = set()
///         for relation in relations:
///             subject = str(relation.get("subject", "")); object_value = str(relation.get("object", ""))
///             if subject in frontier or object_value in frontier:
///                 matched.append(relation)
///                 if subject not in visited: next_frontier.add(subject)
///                 if object_value not in visited: next_frontier.add(object_value)
///         visited.update(next_frontier); frontier = next_frontier
///         if not frontier: break
///     unique = {relation["id"]: relation for relation in matched}
///     return {"node_id": node_id, "depth": depth,
///             "relations": [relation_payload(item) for item in sorted(unique.values(), key=lambda v: v["id"])]}
/// ```
pub fn code_relation_neighborhood(bundle_dir: &Path, node_id: &str, depth: i64) -> Result<Value> {
    let state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_relations.json"),
        json!({"relations": []}),
    );
    let relations = state.get("relations").and_then(Value::as_array).cloned().unwrap_or_default();

    let mut frontier: HashSet<String> = HashSet::from([node_id.to_string()]);
    let mut visited: HashSet<String> = HashSet::from([node_id.to_string()]);
    let mut matched: Vec<Value> = Vec::new();

    for _ in 0..depth.max(1) {
        let mut next_frontier: HashSet<String> = HashSet::new();
        for relation in &relations {
            let subject = relation.get("subject").and_then(Value::as_str).unwrap_or_default().to_string();
            let object = relation.get("object").and_then(Value::as_str).unwrap_or_default().to_string();
            if frontier.contains(&subject) || frontier.contains(&object) {
                matched.push(relation.clone());
                if !visited.contains(&subject) {
                    next_frontier.insert(subject.clone());
                }
                if !visited.contains(&object) {
                    next_frontier.insert(object.clone());
                }
            }
        }
        visited.extend(next_frontier.iter().cloned());
        frontier = next_frontier;
        if frontier.is_empty() {
            break;
        }
    }

    // Python: `{relation["id"]: relation for relation in matched}` then
    // `sorted(unique.values(), key=lambda v: v["id"])` — a BTreeMap keyed by
    // id gives dedupe-by-id plus id-ascending order in one step.
    let mut unique: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();
    for relation in matched {
        let id = relation.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
        unique.insert(id, relation);
    }
    let relations_out: Vec<Value> = unique.into_values().map(|r| relation_payload(&r)).collect();

    Ok(json!({
        "node_id": node_id,
        "depth": depth,
        "relations": relations_out,
    }))
}

/// Port of `kl4a.codekb.context.code_change_impact`.
///
/// ```python
/// def code_change_impact(bundle_dir: Path, symbol_id: str) -> dict[str, Any]:
///     neighborhood = code_relation_neighborhood(bundle_dir, symbol_id, depth=2)
///     impacted = sorted(
///         {str(r["subject"]) for r in neighborhood["relations"] if r["subject"] != symbol_id}
///         | {str(r["object"]) for r in neighborhood["relations"]
///            if r["object"] != symbol_id and str(r["object"]).startswith("symbol-")}
///     )
///     return {"symbol_id": symbol_id, "impacted_symbol_ids": impacted,
///             "relation_count": len(neighborhood["relations"]), "test_symbols": code_tests_for_symbol(bundle_dir, symbol_id)}
/// ```
pub fn code_change_impact(bundle_dir: &Path, symbol_id: &str) -> Result<Value> {
    let neighborhood = code_relation_neighborhood(bundle_dir, symbol_id, 2)?;
    let relations = neighborhood.get("relations").and_then(Value::as_array).cloned().unwrap_or_default();

    let mut impacted: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for relation in &relations {
        let subject = relation.get("subject").and_then(Value::as_str).unwrap_or_default();
        if subject != symbol_id && !subject.is_empty() {
            impacted.insert(subject.to_string());
        }
        let object = relation.get("object").and_then(Value::as_str).unwrap_or_default();
        if object != symbol_id && object.starts_with("symbol-") {
            impacted.insert(object.to_string());
        }
    }

    let relation_count = relations.len();
    let test_symbols = code_tests_for_symbol(bundle_dir, symbol_id)?;

    Ok(json!({
        "symbol_id": symbol_id,
        "impacted_symbol_ids": Value::Array(impacted.into_iter().map(Value::String).collect()),
        "relation_count": relation_count,
        "test_symbols": test_symbols,
    }))
}

/// Port of `kl4a.codekb.context.code_files_search`.
///
/// ```python
/// def code_files_search(bundle_dir: Path, query: str, *, language: str | None=None) -> list[dict[str, Any]]:
///     lowered = _normalize_separators(query)
///     sources = read_json(bundle_dir / STATE_DIR / "code_inventory.json", {"sources": []}).get("sources", [])
///     results = []
///     for source in sources:
///         if language and source.get("language") != language: continue
///         haystack = _normalize_separators(" ".join([source.get("id", ""), source.get("title", ""),
///             source.get("path", ""), source.get("kind", ""), source.get("language", "")]))
///         if lowered in haystack:
///             results.append({"id": source["id"], "title": source["title"], "path": source["path"],
///                 "language": source["language"], "kind": source["kind"], "checksum": source.get("checksum"),
///                 "source_version_id": source.get("source_version_id"), "okf_path": source.get("okf_path")})
///     return sorted(results, key=lambda item: item["path"])
/// ```
pub fn code_files_search(bundle_dir: &Path, query: &str, language: Option<&str>) -> Result<Value> {
    // Fix (Medium finding — CLI empty-string handling): Python's filter is
    // `if language and source.get("language") != language: continue`, and
    // Python's `and`/`or` truthiness treats `""` the same as `None` (both
    // falsy) — so `--language ""` must behave exactly like omitting
    // `--language`, not like filtering for a literal empty-string language
    // (which no source ever has, silently returning zero results). The
    // previous version here passed `Option<&str>` straight through without
    // this normalization, so `Some("")` filtered everything out instead of
    // nothing.
    let language = language.filter(|s| !s.is_empty());
    let lowered = normalize_separators(query);
    let state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_inventory.json"),
        json!({"sources": []}),
    );
    let sources = state.get("sources").and_then(Value::as_array).cloned().unwrap_or_default();

    let mut results: Vec<Value> = Vec::new();
    for source in sources {
        if let Some(lang) = language {
            if source.get("language").and_then(Value::as_str) != Some(lang) {
                continue;
            }
        }
        let haystack_raw = [
            source.get("id").and_then(Value::as_str).unwrap_or_default(),
            source.get("title").and_then(Value::as_str).unwrap_or_default(),
            source.get("path").and_then(Value::as_str).unwrap_or_default(),
            source.get("kind").and_then(Value::as_str).unwrap_or_default(),
            source.get("language").and_then(Value::as_str).unwrap_or_default(),
        ]
        .join(" ");
        let haystack = normalize_separators(&haystack_raw);
        if haystack.contains(lowered.as_str()) {
            results.push(json!({
                "id": source.get("id").cloned().unwrap_or(Value::Null),
                "title": source.get("title").cloned().unwrap_or(Value::Null),
                "path": source.get("path").cloned().unwrap_or(Value::Null),
                "language": source.get("language").cloned().unwrap_or(Value::Null),
                "kind": source.get("kind").cloned().unwrap_or(Value::Null),
                "checksum": source.get("checksum").cloned().unwrap_or(Value::Null),
                "source_version_id": source.get("source_version_id").cloned().unwrap_or(Value::Null),
                "okf_path": source.get("okf_path").cloned().unwrap_or(Value::Null),
            }));
        }
    }
    results.sort_by(|a, b| {
        let pa = a.get("path").and_then(Value::as_str).unwrap_or_default();
        let pb = b.get("path").and_then(Value::as_str).unwrap_or_default();
        pa.cmp(pb)
    });
    Ok(Value::Array(results))
}

/// Port of `kl4a.codekb.context.code_symbols_search`, including its nested
/// `collect(predicate)` closure.
///
/// ```python
/// def code_symbols_search(bundle_dir: Path, query: str, *, language: str | None=None) -> list[dict[str, Any]]:
///     terms = _QUERY_TERM_RE.findall(query.lower())
///     symbols = read_json(bundle_dir / STATE_DIR / "code_symbols.json", {"symbols": []}).get("symbols", [])
///     candidates = []
///     for symbol in symbols:
///         if language and symbol.get("language") != language: continue
///         # Strip the "symbol-" id prefix before scoring — see original docstring
///         # for why: the bare word "symbol" would otherwise match every candidate.
///         id_content = symbol.get("id", "").removeprefix("symbol-")
///         haystack = " ".join([id_content, symbol.get("title", ""), symbol.get("qualified_name", ""),
///             symbol.get("file", ""), symbol.get("kind", ""), symbol.get("docstring") or ""]).lower()
///         name = (symbol.get("qualified_name", "") + " " + id_content).lower()
///         candidates.append((symbol, haystack, name))
///
///     def collect(predicate) -> list[tuple[int, dict[str, Any]]]:
///         found = []
///         for symbol, haystack, name in candidates:
///             hits = [term for term in terms if term in haystack]
///             if predicate(hits):
///                 score = sum(1 for term in terms if term in name) * 100 + len(hits)
///                 found.append((score, symbol))
///         return found
///
///     if not terms:
///         matched = [(0, symbol) for symbol, _h, _n in candidates]
///     else:
///         matched = collect(lambda hits: len(hits) == len(terms))
///         if not matched:
///             matched = collect(lambda hits: len(hits) > 0)
///     matched.sort(key=lambda pair: (-pair[0], pair[1].get("qualified_name", "")))
///     return [symbol_payload(symbol) for _score, symbol in matched]
/// ```
pub fn code_symbols_search(bundle_dir: &Path, query: &str, language: Option<&str>) -> Result<Value> {
    // Fix (Medium finding — CLI empty-string handling): same Python
    // `if language and ...` truthiness as `code_files_search` above —
    // `Some("")` must behave like `None`, not like a literal (unmatchable)
    // language filter.
    let language = language.filter(|s| !s.is_empty());
    let lowered_query = query.to_lowercase();
    let terms: Vec<String> = QUERY_TERM_RE.find_iter(&lowered_query).map(|m| m.as_str().to_string()).collect();

    let state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_symbols.json"),
        json!({"symbols": []}),
    );
    let symbols = state.get("symbols").and_then(Value::as_array).cloned().unwrap_or_default();

    struct Candidate {
        symbol: Value,
        haystack: String,
        name: String,
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    for symbol in symbols {
        if let Some(lang) = language {
            if symbol.get("language").and_then(Value::as_str) != Some(lang) {
                continue;
            }
        }
        let id = symbol.get("id").and_then(Value::as_str).unwrap_or_default();
        let id_content = id.strip_prefix("symbol-").unwrap_or(id);
        let haystack = [
            id_content,
            symbol.get("title").and_then(Value::as_str).unwrap_or_default(),
            symbol.get("qualified_name").and_then(Value::as_str).unwrap_or_default(),
            symbol.get("file").and_then(Value::as_str).unwrap_or_default(),
            symbol.get("kind").and_then(Value::as_str).unwrap_or_default(),
            symbol.get("docstring").and_then(Value::as_str).unwrap_or_default(),
        ]
        .join(" ")
        .to_lowercase();
        let name = format!(
            "{} {}",
            symbol.get("qualified_name").and_then(Value::as_str).unwrap_or_default(),
            id_content
        )
        .to_lowercase();
        candidates.push(Candidate { symbol, haystack, name });
    }

    let collect = |predicate: &dyn Fn(usize) -> bool| -> Vec<(i64, Value)> {
        let mut found = Vec::new();
        for c in &candidates {
            let hits = terms.iter().filter(|t| c.haystack.contains(t.as_str())).count();
            if predicate(hits) {
                let name_hits = terms.iter().filter(|t| c.name.contains(t.as_str())).count() as i64;
                let score = name_hits * 100 + hits as i64;
                found.push((score, c.symbol.clone()));
            }
        }
        found
    };

    let mut matched: Vec<(i64, Value)> = if terms.is_empty() {
        candidates.iter().map(|c| (0i64, c.symbol.clone())).collect()
    } else {
        let terms_len = terms.len();
        let mut m = collect(&|hits| hits == terms_len);
        if m.is_empty() {
            m = collect(&|hits| hits > 0);
        }
        m
    };

    matched.sort_by(|a, b| {
        // Python: `key=lambda pair: (-pair[0], pair[1].get("qualified_name", ""))`
        b.0.cmp(&a.0).then_with(|| {
            let qa = a.1.get("qualified_name").and_then(Value::as_str).unwrap_or_default();
            let qb = b.1.get("qualified_name").and_then(Value::as_str).unwrap_or_default();
            qa.cmp(qb)
        })
    });

    Ok(Value::Array(matched.into_iter().map(|(_, s)| symbol_payload(&s)).collect()))
}

/// Port of `kl4a.codekb.context.code_relations_search`.
///
/// ```python
/// def code_relations_search(bundle_dir: Path, *, subject="", predicate="", object_text="", resolution_status="") -> list[dict[str, Any]]:
///     relations = read_json(bundle_dir / STATE_DIR / "code_relations.json", {"relations": []}).get("relations", [])
///     results = []
///     for relation in relations:
///         if subject and _normalize_separators(subject) not in _normalize_separators(str(relation.get("subject", ""))): continue
///         if predicate and predicate.lower() != str(relation.get("predicate", "")).lower(): continue
///         if object_text and _normalize_separators(object_text) not in _normalize_separators(str(relation.get("object", ""))): continue
///         if resolution_status and resolution_status.lower() != str((relation.get("relation") or {}).get("resolution_status", "")).lower(): continue
///         results.append(relation_payload(relation))
///     return sorted(results, key=lambda item: (item["predicate"], item["subject"], item["object"]))
/// ```
pub fn code_relations_search(
    bundle_dir: &Path,
    subject: &str,
    predicate: &str,
    object_text: &str,
    resolution_status: &str,
) -> Result<Value> {
    let state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_relations.json"),
        json!({"relations": []}),
    );
    let relations = state.get("relations").and_then(Value::as_array).cloned().unwrap_or_default();

    let mut results: Vec<Value> = Vec::new();
    for relation in relations {
        if !subject.is_empty() {
            let rel_subject = relation.get("subject").and_then(Value::as_str).unwrap_or_default();
            if !normalize_separators(rel_subject).contains(normalize_separators(subject).as_str()) {
                continue;
            }
        }
        if !predicate.is_empty() {
            let rel_predicate = relation.get("predicate").and_then(Value::as_str).unwrap_or_default();
            if predicate.to_lowercase() != rel_predicate.to_lowercase() {
                continue;
            }
        }
        if !object_text.is_empty() {
            let rel_object = relation.get("object").and_then(Value::as_str).unwrap_or_default();
            if !normalize_separators(rel_object).contains(normalize_separators(object_text).as_str()) {
                continue;
            }
        }
        if !resolution_status.is_empty() {
            let rel_status = relation
                .get("relation")
                .and_then(Value::as_object)
                .and_then(|r| r.get("resolution_status"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            if resolution_status.to_lowercase() != rel_status.to_lowercase() {
                continue;
            }
        }
        results.push(relation_payload(&relation));
    }
    sort_by_predicate_subject_object(&mut results);
    Ok(Value::Array(results))
}

/// Port of `kl4a.codekb.context.code_symbol_get`.
///
/// ```python
/// def code_symbol_get(bundle_dir: Path, symbol_id: str) -> dict[str, Any]:
///     symbols_state = read_json(bundle_dir / STATE_DIR / "code_symbols.json", {"symbols": [], "evidence": []})
///     symbol = next((i for i in symbols_state.get("symbols", []) if i.get("id") == symbol_id), None)
///     if symbol is None:
///         raise KeyError(f"unknown code symbol: {symbol_id}")
///     evidence = next((i for i in symbols_state.get("evidence", []) if i.get("id") == symbol.get("evidence_id")), None)
///     relations = code_relations_search(bundle_dir, subject=symbol_id)
///     knowledge = [item_payload(i) for i in read_json(bundle_dir / STATE_DIR / "code_knowledge.json", {"items": []}).get("items", [])
///                  if symbol_id in i.get("symbols", [])]
///     return {**symbol_payload(symbol), "evidence": evidence, "relations": relations, "knowledge": knowledge}
/// ```
pub fn code_symbol_get(bundle_dir: &Path, symbol_id: &str) -> Result<Value> {
    let symbols_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_symbols.json"),
        json!({"symbols": [], "evidence": []}),
    );
    let symbols = symbols_state.get("symbols").and_then(Value::as_array).cloned().unwrap_or_default();
    let symbol = symbols
        .iter()
        .find(|s| s.get("id").and_then(Value::as_str) == Some(symbol_id));
    let symbol = match symbol {
        Some(s) => s,
        None => bail!("unknown code symbol: {symbol_id}"),
    };

    let evidence_id = symbol.get("evidence_id").and_then(Value::as_str);
    let evidence_list = symbols_state.get("evidence").and_then(Value::as_array).cloned().unwrap_or_default();
    let evidence = evidence_id.and_then(|eid| {
        evidence_list
            .into_iter()
            .find(|e| e.get("id").and_then(Value::as_str) == Some(eid))
    });

    let relations = code_relations_search(bundle_dir, symbol_id, "", "", "")?;

    let knowledge_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_knowledge.json"),
        json!({"items": []}),
    );
    let knowledge: Vec<Value> = knowledge_state
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|item| {
            item.get("symbols")
                .and_then(Value::as_array)
                .map(|syms| syms.iter().any(|s| s.as_str() == Some(symbol_id)))
                .unwrap_or(false)
        })
        .map(|item| item_payload(&item))
        .collect();

    let mut payload = symbol_payload(symbol);
    if let Value::Object(map) = &mut payload {
        map.insert("evidence".to_string(), evidence.unwrap_or(Value::Null));
        map.insert("relations".to_string(), relations);
        map.insert("knowledge".to_string(), Value::Array(knowledge));
    }
    Ok(payload)
}

/// Port of `kl4a.codekb.context.code_repo_describe`.
///
/// ```python
/// def code_repo_describe(bundle_dir: Path) -> dict[str, Any]:
///     inventory = read_json(bundle_dir / STATE_DIR / "code_inventory.json", {"repository": {}, "sources": [], "warnings": []})
///     symbols_state = read_json(bundle_dir / STATE_DIR / "code_symbols.json", {"modules": [], "symbols": [], "evidence": []})
///     relations_state = read_json(bundle_dir / STATE_DIR / "code_relations.json", {"relations": []})
///     knowledge_state = read_json(bundle_dir / STATE_DIR / "code_knowledge.json", {"items": []})
///     return {"repository": inventory.get("repository", {}), "source_count": len(inventory.get("sources", [])),
///             "detected_languages": inventory.get("detected_languages", {}), "module_count": len(symbols_state.get("modules", [])),
///             "symbol_count": len(symbols_state.get("symbols", [])), "relation_count": len(relations_state.get("relations", [])),
///             "code_knowledge_count": len(knowledge_state.get("items", [])), "warning_count": len(inventory.get("warnings", []))}
/// ```
pub fn code_repo_describe(bundle_dir: &Path) -> Result<Value> {
    let inventory = read_json(
        &bundle_dir.join(STATE_DIR).join("code_inventory.json"),
        json!({"repository": {}, "sources": [], "warnings": []}),
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

    let count = |v: &Value, key: &str| -> usize { v.get(key).and_then(Value::as_array).map(Vec::len).unwrap_or(0) };

    Ok(json!({
        "repository": inventory.get("repository").cloned().unwrap_or_else(|| Value::Object(Map::new())),
        "source_count": count(&inventory, "sources"),
        "detected_languages": inventory.get("detected_languages").cloned().unwrap_or_else(|| Value::Object(Map::new())),
        "module_count": count(&symbols_state, "modules"),
        "symbol_count": count(&symbols_state, "symbols"),
        "relation_count": count(&relations_state, "relations"),
        "code_knowledge_count": count(&knowledge_state, "items"),
        "warning_count": count(&inventory, "warnings"),
    }))
}

/// Port of `kl4a.codekb.context.code_context`.
///
/// ```python
/// def code_context(bundle_dir: Path, *, task: str, query: str | None=None, language: str | None=None) -> dict[str, Any]:
///     search_query = query or task.replace("-", " ")
///     symbols = code_symbols_search(bundle_dir, search_query, language=language)
///     if not symbols:
///         symbols = code_symbols_search(bundle_dir, "", language=language)
///     symbols = symbols[:_CONTEXT_SYMBOL_LIMIT]
///     evidence_by_id = {item["id"]: item for item in read_json(bundle_dir / STATE_DIR / "code_symbols.json", {"evidence": []}).get("evidence", [])}
///     symbol_ids = {symbol["id"] for symbol in symbols}
///     relations = relations_for_symbols(bundle_dir, symbol_ids)
///     related_symbol_ids = {value for relation in relations for value in (relation.get("subject"), relation.get("object"))
///                            if isinstance(value, str) and value.startswith("symbol-")}
///     selected_ids = symbol_ids | related_symbol_ids
///     evidence = [evidence_by_id[symbol["evidence_id"]] for symbol in symbols if symbol.get("evidence_id") in evidence_by_id]
///     knowledge = [item_payload(item) for item in reviewed_code_items(bundle_dir)
///                  if set(item.get("symbols", [])) & selected_ids and item_is_default_usable(item)]
///     symbols_by_id, tests_by_symbol_id = _covers_index(bundle_dir)
///     tests = []
///     for symbol_id in sorted(selected_ids):
///         tests.extend(_tests_for_symbol_from_index(symbol_id, symbols_by_id, tests_by_symbol_id))
///     return {
///         "task": {"id": task, "query": search_query},
///         "context_rules": [
///             "Use returned symbols and evidence as code KB context.",
///             "Treat grammar facts as parser-derived and exact only when evidence span_status is exact.",
///             "Do not treat behavior or architecture claims as approved unless review status permits it.",
///         ],
///         "symbols": symbols, "evidence": evidence,
///         "relations": [relation_payload(r) for r in relations][:_CONTEXT_LIST_LIMIT],
///         "tests": dedupe_payloads(tests)[:_CONTEXT_LIST_LIMIT],
///         "code_knowledge": knowledge[:_CONTEXT_LIST_LIMIT],
///         "warnings": context_warnings(relations, knowledge),
///     }
/// ```
pub fn code_context(bundle_dir: &Path, task: &str, query: Option<&str>, language: Option<&str>) -> Result<Value> {
    // Fix (Medium finding — CLI empty-string handling): Python computes
    // `search_query = query or task.replace("-", " ")`, and `""` is falsy
    // in Python, so an empty `--query` falls back to `task` exactly like an
    // omitted one. The previous version here only fell back on `None`
    // (`query.map(...).unwrap_or_else(...)`), so `--query ""` incorrectly
    // searched for a literal empty string instead of falling back to the
    // task text.
    let search_query = query
        .filter(|q| !q.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| task.replace('-', " "));

    let mut symbols: Vec<Value> = code_symbols_search(bundle_dir, &search_query, language)?
        .as_array()
        .cloned()
        .unwrap_or_default();
    if symbols.is_empty() {
        symbols = code_symbols_search(bundle_dir, "", language)?
            .as_array()
            .cloned()
            .unwrap_or_default();
    }
    symbols.truncate(CONTEXT_SYMBOL_LIMIT);

    let symbols_state_evidence = read_json(
        &bundle_dir.join(STATE_DIR).join("code_symbols.json"),
        json!({"evidence": []}),
    );
    let evidence_by_id: HashMap<String, Value> = symbols_state_evidence
        .get("evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| e.get("id").and_then(Value::as_str).map(|id| (id.to_string(), e.clone())))
        .collect();

    let symbol_ids: HashSet<String> = symbols
        .iter()
        .filter_map(|s| s.get("id").and_then(Value::as_str))
        .map(String::from)
        .collect();
    let relations = relations_for_symbols(bundle_dir, &symbol_ids)?;

    let mut related_symbol_ids: HashSet<String> = HashSet::new();
    for relation in &relations {
        for key in ["subject", "object"] {
            if let Some(v) = relation.get(key).and_then(Value::as_str) {
                if v.starts_with("symbol-") {
                    related_symbol_ids.insert(v.to_string());
                }
            }
        }
    }
    let mut selected_ids: HashSet<String> = symbol_ids;
    selected_ids.extend(related_symbol_ids);

    let evidence: Vec<Value> = symbols
        .iter()
        .filter_map(|symbol| {
            symbol
                .get("evidence_id")
                .and_then(Value::as_str)
                .and_then(|eid| evidence_by_id.get(eid).cloned())
        })
        .collect();

    let reviewed = reviewed_code_items(bundle_dir)?;
    let mut knowledge: Vec<Value> = reviewed
        .into_iter()
        .filter(|item| {
            let syms: HashSet<String> = item
                .get("symbols")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            !syms.is_disjoint(&selected_ids) && item_is_default_usable(&item)
        })
        .map(|item| item_payload(&item))
        .collect();

    let (symbols_by_id, tests_by_symbol_id) = covers_index(bundle_dir)?;
    let mut sorted_selected: Vec<&String> = selected_ids.iter().collect();
    sorted_selected.sort();
    let mut tests: Vec<Value> = Vec::new();
    for sid in sorted_selected {
        tests.extend(tests_for_symbol_from_index(sid, &symbols_by_id, &tests_by_symbol_id));
    }

    let mut relation_payloads: Vec<Value> = relations.iter().map(relation_payload).collect();
    relation_payloads.truncate(CONTEXT_LIST_LIMIT);

    let warnings = context_warnings(&relations, &knowledge);

    let mut deduped_tests = dedupe_payloads(tests);
    deduped_tests.truncate(CONTEXT_LIST_LIMIT);
    knowledge.truncate(CONTEXT_LIST_LIMIT);

    Ok(json!({
        "task": {
            "id": task,
            "query": search_query,
        },
        "context_rules": [
            "Use returned symbols and evidence as code KB context.",
            "Treat grammar facts as parser-derived and exact only when evidence span_status is exact.",
            "Do not treat behavior or architecture claims as approved unless review status permits it.",
        ],
        "symbols": symbols,
        "evidence": evidence,
        "relations": relation_payloads,
        "tests": deduped_tests,
        "code_knowledge": knowledge,
        "warnings": warnings,
    }))
}
