//! Port of `kl4a/codekb/knowledge.py` -- the code-knowledge mining pipeline:
//! static-graph claim extraction plus an optional LLM narrative pass.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use once_cell::sync::Lazy;
use serde_json::{Map, Value};

use crate::lifecycle::merge_code_items;
// WIRING: `read_json(path: &Path, default: Value) -> Value` was confirmed via
// MCP against kl4a/codekb/state.py -- it tolerates a missing/torn/empty file
// by returning `default` and never raises, so it does NOT return a `Result`
// here (no `?` on its call sites below). `write_code_state` and `STATE_DIR`
// were not independently re-verified in this batch beyond this call site
// (STATE_DIR's value `.codekb` was confirmed elsewhere);
// `write_code_state(bundle_dir, name, data) -> Result<()>` is the shape
// `mine_code_bundle` calls it with in Python and is UNCONFIRMED beyond that.
use crate::state::{read_json, write_code_state, STATE_DIR};

// WIRING: the following are out of this batch's scope (`kl4a/codekb/{knowledge,
// layout,lifecycle,model}.py` only) and are expected to come from sibling
// batches. Signatures below are best-effort reconstructions from how
// knowledge.py calls them and are marked UNCONFIRMED where the exact Rust
// shape could not be grounded from this batch's MCP queries; reconcile
// against the real modules once they land.
//
// - `crate::author`: CodeAuthorFn, azure_code_author, build_symbol_author_request,
//   CODE_AUTHOR_SYSTEM_PROMPT, validate_candidate, code_knowledge_id_for_claim
//   (all confirmed to exist in kl4a/codekb/author.py; exact Rust signatures
//   UNCONFIRMED).
// - `crate::cache`: EnrichmentCache, fingerprint (confirmed to exist in
//   kl4a/codekb/cache.py; exact Rust API UNCONFIRMED).
// - `crate::procedures`: cluster_symbols, procedures_enabled, procedures_min_calls,
//   static_procedure_claims, enrich_with_procedure_author, azure_procedure_author
//   (confirmed to exist in kl4a/codekb/procedures.py; a sibling procedures.rs
//   is listed in this repo's git status as already in progress).
// - `crate::ids::code_knowledge_id_for` (confirmed in kl4a/codekb/ids.py).
// - `crate::bundle_store::load_manifest` and `crate::okf_writer::write_markdown`:
//   UNCONFIRMED home. Python has these in a shared `kl4a.kl4a` package above
//   codekb/apikb, not inside kl4a/codekb -- this Rust workspace has no
//   equivalent shared crate yet. Needs a real module (likely a new
//   `kl4a-core` crate) wired in by the coordinator.
use crate::author::{
    azure_code_author, build_symbol_author_request, code_knowledge_id_for_claim,
    validate_candidate, CodeAuthorFn, CODE_AUTHOR_SYSTEM_PROMPT,
};
use crate::bundle_store::load_manifest;
use crate::cache::{fingerprint, EnrichmentCache};
use crate::ids::code_knowledge_id_for;
use crate::okf_writer::write_markdown;
use crate::procedures::{
    azure_procedure_author, cluster_symbols, enrich_with_procedure_author, procedures_enabled,
    procedures_min_calls, static_procedure_claims,
};

/// Mirrors `kl4a.codekb.knowledge.STATIC_PROVIDERS`.
static STATIC_PROVIDERS: Lazy<HashSet<String>> = Lazy::new(|| {
    ["fixture", "static"].iter().map(|s| s.to_string()).collect()
});

/// Mirrors `kl4a.codekb.knowledge.HYBRID_PROVIDERS`.
///
/// `HYBRID_PROVIDERS = {"hybrid", "llm", "azure-llm"} | set(LLM_PROVIDER_IDS)`
/// (`kl4a/codekb/knowledge.py:40`). `LLM_PROVIDER_IDS` itself is not a symbol
/// that exists anywhere in the indexed corpus under that name (confirmed via
/// `code_relations_search`/`code_symbols_search` for it directly, and via the
/// full `defines` list for `kl4a/kl4a/llm_provider.py`, which has no such
/// constant) — it is `knowledge.py`'s import-time name for
/// `kl4a.kl4a.llm_provider.PROVIDERS` (`kl4a/kl4a/llm_provider.py:398-402`),
/// a `dict[str, type[Provider]]` keyed by each provider's `id`:
/// `AzureResponsesProvider.id` = `"azure-responses"`,
/// `OpenAICompatibleProvider.id` = `"openai-compatible"`,
/// `AnthropicProvider.id` = `"anthropic"`. Python's `set(some_dict)` yields
/// the dict's keys, so `set(LLM_PROVIDER_IDS)` is exactly that id set.
pub static HYBRID_PROVIDERS: Lazy<HashSet<String>> = Lazy::new(|| {
    [
        "hybrid",
        "llm",
        "azure-llm",
        "azure-responses",
        "openai-compatible",
        "anthropic",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
});

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None => false,
        Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// Mirrors Python's `int(value)` coercion as used by `mining_max_workers`
/// (new) and the pre-existing `demote_unanchored_to_tier` lookup: a decimal
/// string like `"3.5"` is NOT accepted by Python's `int()` (raises
/// `ValueError`, which both call sites catch and fall back to their
/// default) -- unlike a float *value* (`3.5`), which `int()` truncates
/// toward zero without error. Only the string branch previously had an
/// extra float-parse fallback that let a string like `"3.5"` through as
/// `3`; removed so a quoted decimal in config falls back to the same
/// default Python would use instead of silently truncating.
fn value_as_int(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        Value::Bool(b) => Some(if *b { 1 } else { 0 }),
        _ => None,
    }
}

fn sorted_unique_str_array(v: Option<&Value>) -> Value {
    let set: BTreeSet<String> = v
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    Value::Array(set.into_iter().map(Value::String).collect())
}

/// Mirrors `mining_config`: `((manifest.get("codekb") or {}).get("mining")) or {}`,
/// swallowing any `load_manifest` failure into `{}`.
pub fn mining_config(bundle_dir: &Path) -> Value {
    let manifest = match load_manifest(bundle_dir) {
        Ok(m) => m,
        Err(_) => return Value::Object(Map::new()),
    };
    let codekb = manifest
        .get("codekb")
        .filter(|v| truthy(Some(v)))
        .cloned()
        .unwrap_or(Value::Object(Map::new()));
    codekb
        .get("mining")
        .filter(|v| truthy(Some(v)))
        .cloned()
        .unwrap_or(Value::Object(Map::new()))
}

/// Mirrors `mining_selection`.
pub fn mining_selection(bundle_dir: &Path) -> Value {
    mining_config(bundle_dir)
        .get("selection")
        .filter(|v| truthy(Some(v)))
        .cloned()
        .unwrap_or(Value::Object(Map::new()))
}

/// Mirrors `mining_grounding`.
pub fn mining_grounding(bundle_dir: &Path) -> Value {
    mining_config(bundle_dir)
        .get("grounding")
        .filter(|v| truthy(Some(v)))
        .cloned()
        .unwrap_or(Value::Object(Map::new()))
}

/// Mirrors `mining_cache_enabled`: whether enrichment responses may be
/// reused. Defaults to on.
pub fn mining_cache_enabled(bundle_dir: &Path) -> bool {
    match mining_config(bundle_dir).get("cache") {
        Some(Value::Object(obj)) => match obj.get("enabled") {
            Some(v) => truthy(Some(v)),
            None => true,
        },
        _ => true,
    }
}

/// Mirrors `mining_max_workers`: how many hybrid-authoring LLM calls may be
/// in flight at once. Defaults to 5: enough to meaningfully cut wall-clock
/// time on a large mine run without hammering the provider hard enough to
/// trip rate limits the way an unbounded/fully-parallel run could.
pub fn mining_max_workers(bundle_dir: &Path) -> i64 {
    let value = mining_config(bundle_dir).get("max_workers").cloned();
    match value.as_ref().and_then(value_as_int) {
        Some(n) => n.max(1),
        None => 5,
    }
}

/// Mirrors `group_relations_by_subject`.
pub fn group_relations_by_subject(relations: &[Value]) -> HashMap<String, Vec<Value>> {
    let mut grouped: HashMap<String, Vec<Value>> = HashMap::new();
    for relation in relations {
        if let Some(subject) = relation.get("subject").and_then(Value::as_str) {
            grouped
                .entry(subject.to_string())
                .or_default()
                .push(relation.clone());
        }
    }
    grouped
}

/// Mirrors `operational_signal`.
pub fn operational_signal(symbol: &Value, relations: &[Value]) -> bool {
    const TERMS: [&str; 7] = [
        "pipeline",
        "ingest",
        "preview",
        "workflow",
        "validate",
        "authorization",
        "eligibility",
    ];
    let predicates_joined = relations
        .iter()
        .map(|r| r.get("predicate").and_then(Value::as_str).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ");
    let haystack = [
        symbol.get("qualified_name").and_then(Value::as_str).unwrap_or(""),
        symbol.get("title").and_then(Value::as_str).unwrap_or(""),
        symbol.get("docstring").and_then(Value::as_str).unwrap_or(""),
        predicates_joined.as_str(),
    ]
    .join(" ")
    .to_lowercase();
    TERMS.iter().any(|term| haystack.contains(term))
}

/// Mirrors `knowledge_item`: build a purely-static (fixture-derived) OKF
/// knowledge item.
#[allow(clippy::too_many_arguments)]
pub fn knowledge_item(
    symbol: &Value,
    suffix: &str,
    title: &str,
    claim: &str,
    tier: i64,
    derivation: &str,
    relation_ids: &[String],
    review_required: bool,
) -> Value {
    let symbol_id = symbol.get("id").and_then(Value::as_str).unwrap_or("").to_string();
    let item_id = code_knowledge_id_for(&symbol_id, suffix);
    let evidence_ids: Vec<Value> = symbol
        .get("evidence_id")
        .filter(|v| truthy(Some(v)))
        .map(|v| vec![v.clone()])
        .unwrap_or_default();
    let relations_sorted: BTreeSet<String> = relation_ids.iter().cloned().collect();

    let mut out = Map::new();
    out.insert("id".into(), Value::String(item_id.clone()));
    out.insert("type".into(), Value::String("Code Knowledge".into()));
    out.insert("title".into(), Value::String(title.to_string()));
    out.insert("claim".into(), Value::String(claim.to_string()));
    out.insert("knowledge_tier".into(), Value::from(tier));
    out.insert("review_status".into(), Value::String("proposed".into()));
    out.insert("lifecycle_status".into(), Value::String("active".into()));
    out.insert("confidence".into(), Value::from(if tier == 4 { 0.72 } else { 0.82 }));
    out.insert("review_required".into(), Value::Bool(review_required || tier == 4));
    out.insert(
        "evidence_status".into(),
        Value::String(if !evidence_ids.is_empty() { "linked".into() } else { "unresolved".into() }),
    );
    out.insert("evidence".into(), Value::Array(evidence_ids));
    out.insert(
        "relations".into(),
        Value::Array(relations_sorted.into_iter().map(Value::String).collect()),
    );
    out.insert("symbols".into(), Value::Array(vec![Value::String(symbol_id.clone())]));

    let mut code = Map::new();
    code.insert(
        "language".into(),
        symbol.get("language").cloned().unwrap_or_else(|| Value::String("python".into())),
    );
    code.insert("symbol_id".into(), Value::String(symbol_id));
    code.insert("qualified_name".into(), symbol.get("qualified_name").cloned().unwrap_or(Value::Null));
    code.insert("file".into(), symbol.get("file").cloned().unwrap_or(Value::Null));
    code.insert(
        "source_version_id".into(),
        symbol.get("source_version_id").cloned().unwrap_or(Value::Null),
    );
    code.insert("derivation".into(), Value::String(derivation.to_string()));
    code.insert("provider".into(), Value::String("codekb/static".into()));
    code.insert("anchor_status".into(), Value::String("exact".into()));
    out.insert("code".into(), Value::Object(code));

    out.insert("okf_path".into(), Value::String(format!("knowledge/{item_id}.md")));
    Value::Object(out)
}

/// Mirrors `cobol_symbol_claims`.
pub fn cobol_symbol_claims(symbol: &Value, relations: &[Value]) -> Vec<Value> {
    let mut claims = Vec::new();
    let predicates: HashSet<String> = relations
        .iter()
        .filter_map(|r| r.get("predicate").and_then(Value::as_str).map(|s| s.to_string()))
        .collect();
    let qualified_name = symbol.get("qualified_name").and_then(Value::as_str).unwrap_or("");

    let relation_ids_for = |wanted: &[&str]| -> Vec<String> {
        relations
            .iter()
            .filter(|r| {
                r.get("predicate")
                    .and_then(Value::as_str)
                    .map(|p| wanted.contains(&p))
                    .unwrap_or(false)
            })
            .filter_map(|r| r.get("id").and_then(Value::as_str).map(|s| s.to_string()))
            .collect()
    };

    let workflow: BTreeSet<&str> = ["performs", "calls"]
        .into_iter()
        .filter(|p| predicates.contains(*p))
        .collect();
    if !workflow.is_empty() {
        claims.push(knowledge_item(
            symbol,
            "cobol-workflow",
            &format!("{qualified_name} participates in COBOL workflow"),
            &format!(
                "{qualified_name} has COBOL workflow relation(s): {}.",
                workflow.into_iter().collect::<Vec<_>>().join(", ")
            ),
            4,
            "fixture-cobol-workflow-relations",
            &relation_ids_for(&["performs", "calls"]),
            true,
        ));
    }

    let io: BTreeSet<&str> = ["reads", "writes"]
        .into_iter()
        .filter(|p| predicates.contains(*p))
        .collect();
    if !io.is_empty() {
        claims.push(knowledge_item(
            symbol,
            "cobol-file-io",
            &format!("{qualified_name} performs COBOL file I/O"),
            &format!(
                "{qualified_name} has COBOL file I/O relation(s): {}.",
                io.into_iter().collect::<Vec<_>>().join(", ")
            ),
            3,
            "fixture-cobol-file-io-relations",
            &relation_ids_for(&["reads", "writes"]),
            false,
        ));
    }

    if predicates.contains("updates") {
        claims.push(knowledge_item(
            symbol,
            "cobol-data-updates",
            &format!("{qualified_name} updates COBOL data"),
            &format!("{qualified_name} updates data item(s) according to line-structured COBOL analysis."),
            3,
            "fixture-cobol-update-relations",
            &relation_ids_for(&["updates"]),
            false,
        ));
    }

    if predicates.contains("branches_on") {
        claims.push(knowledge_item(
            symbol,
            "cobol-branching",
            &format!("{qualified_name} branches on a COBOL condition"),
            &format!("{qualified_name} contains conditional COBOL logic and should be reviewed as behavior."),
            3,
            "fixture-cobol-branch-relations",
            &relation_ids_for(&["branches_on"]),
            false,
        ));
    }

    claims
}

/// Mirrors `symbol_claims`: the per-symbol static claim dispatcher. Every
/// branch of the original (COBOL delegation, docstring/exception/env/test
/// heuristics, the cross-kind operational-signal check) is ported -- none
/// were dropped.
pub fn symbol_claims(symbol: &Value, relations: &[Value]) -> Vec<Value> {
    let mut claims: Vec<Value> = Vec::new();
    let language = symbol.get("language").and_then(Value::as_str).unwrap_or("");
    if language == "cobol" {
        claims.extend(cobol_symbol_claims(symbol, relations));
    }

    let kind = symbol.get("kind").and_then(Value::as_str).unwrap_or("");
    let qualified_name = symbol.get("qualified_name").and_then(Value::as_str).unwrap_or("");

    if matches!(kind, "function" | "method" | "test") {
        let docstring = symbol
            .get("docstring")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !docstring.is_empty() {
            let relation_ids: Vec<String> = relations
                .iter()
                .filter(|r| {
                    matches!(
                        r.get("predicate").and_then(Value::as_str),
                        Some("calls") | Some("raises") | Some("reads") | Some("writes")
                    )
                })
                .filter_map(|r| r.get("id").and_then(Value::as_str).map(|s| s.to_string()))
                .collect();
            claims.push(knowledge_item(
                symbol,
                "documents-behavior",
                &format!("{qualified_name} documents behavior"),
                &format!("{qualified_name} documents this behavior: {docstring}"),
                3,
                "fixture-docstring",
                &relation_ids,
                false,
            ));
        }

        if relations
            .iter()
            .any(|r| r.get("predicate").and_then(Value::as_str) == Some("raises"))
        {
            let exceptions: BTreeSet<String> = relations
                .iter()
                .filter(|r| r.get("predicate").and_then(Value::as_str) == Some("raises"))
                .filter_map(|r| r.get("object").and_then(Value::as_str).map(|s| s.to_string()))
                .collect();
            let relation_ids: Vec<String> = relations
                .iter()
                .filter(|r| r.get("predicate").and_then(Value::as_str) == Some("raises"))
                .filter_map(|r| r.get("id").and_then(Value::as_str).map(|s| s.to_string()))
                .collect();
            claims.push(knowledge_item(
                symbol,
                "raises-exception",
                &format!("{qualified_name} raises exceptions"),
                &format!(
                    "{qualified_name} raises {} according to static analysis.",
                    exceptions.into_iter().collect::<Vec<_>>().join(", ")
                ),
                3,
                "fixture-static-relations",
                &relation_ids,
                false,
            ));
        }

        let has_env_read = relations.iter().any(|r| {
            r.get("predicate").and_then(Value::as_str) == Some("reads")
                && r.get("object")
                    .and_then(Value::as_str)
                    .map(|s| s.starts_with("env:"))
                    .unwrap_or(false)
        });
        if has_env_read {
            let env_vars: BTreeSet<String> = relations
                .iter()
                .filter(|r| {
                    r.get("predicate").and_then(Value::as_str) == Some("reads")
                        && r.get("object")
                            .and_then(Value::as_str)
                            .map(|s| s.starts_with("env:"))
                            .unwrap_or(false)
                })
                .filter_map(|r| {
                    r.get("object")
                        .and_then(Value::as_str)
                        .map(|s| s.strip_prefix("env:").unwrap_or(s).to_string())
                })
                .collect();
            let relation_ids: Vec<String> = relations
                .iter()
                .filter(|r| r.get("predicate").and_then(Value::as_str) == Some("reads"))
                .filter_map(|r| r.get("id").and_then(Value::as_str).map(|s| s.to_string()))
                .collect();
            claims.push(knowledge_item(
                symbol,
                "reads-environment",
                &format!("{qualified_name} reads environment configuration"),
                &format!(
                    "{qualified_name} reads environment variable(s): {}.",
                    env_vars.into_iter().collect::<Vec<_>>().join(", ")
                ),
                3,
                "fixture-static-relations",
                &relation_ids,
                false,
            ));
        }

        if kind == "test" {
            let relation_ids: Vec<String> = relations
                .iter()
                .filter(|r| r.get("predicate").and_then(Value::as_str) == Some("covers"))
                .filter_map(|r| r.get("id").and_then(Value::as_str).map(|s| s.to_string()))
                .collect();
            claims.push(knowledge_item(
                symbol,
                "test-behavior",
                &format!("{qualified_name} is a test behavior claim"),
                &format!(
                    "{qualified_name} is test code and should be used as evidence for related covered symbols when coverage relations exist."
                ),
                3,
                "fixture-test-heuristic",
                &relation_ids,
                false,
            ));
        }
    }

    if matches!(kind, "class" | "function" | "method") && operational_signal(symbol, relations) {
        let relation_ids: Vec<String> = relations
            .iter()
            .filter_map(|r| r.get("id").and_then(Value::as_str).map(|s| s.to_string()))
            .collect();
        claims.push(knowledge_item(
            symbol,
            "architecture-review-required",
            &format!("{qualified_name} may affect workflow behavior"),
            &format!(
                "{qualified_name} has workflow-like naming, docstrings, or relations and needs human review before it is treated as architecture or workflow guidance."
            ),
            4,
            "fixture-workflow-heuristic",
            &relation_ids,
            true,
        ));
    }

    claims
}

/// Mirrors `dedupe_items`: collapse duplicate ids, union their
/// `relations`/`evidence`, and return sorted by id.
pub fn dedupe_items(items: Vec<Value>) -> Vec<Value> {
    let mut by_id: HashMap<String, Value> = HashMap::new();
    for item in items {
        let id = item.get("id").and_then(Value::as_str).unwrap_or("").to_string();
        match by_id.get_mut(&id) {
            None => {
                by_id.insert(id, item);
            }
            Some(existing) => {
                union_str_array_field(existing, &item, "relations");
                union_str_array_field(existing, &item, "evidence");
            }
        }
    }
    let mut result: Vec<Value> = by_id.into_values().collect();
    result.sort_by(|a, b| {
        let ia = a.get("id").and_then(Value::as_str).unwrap_or("");
        let ib = b.get("id").and_then(Value::as_str).unwrap_or("");
        ia.cmp(ib)
    });
    result
}

fn union_str_array_field(existing: &mut Value, item: &Value, field: &str) {
    let mut set: BTreeSet<String> = existing
        .get(field)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if let Some(arr) = item.get(field).and_then(Value::as_array) {
        set.extend(arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())));
    }
    if let Some(obj) = existing.as_object_mut() {
        obj.insert(field.to_string(), Value::Array(set.into_iter().map(Value::String).collect()));
    }
}

/// Mirrors `knowledge_summary`.
pub fn knowledge_summary(items: &[Value]) -> Value {
    let mut by_tier: Map<String, Value> = Map::new();
    let mut review_required: i64 = 0;
    for item in items {
        let tier_key = match item.get("knowledge_tier") {
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
        let count = by_tier.get(&tier_key).and_then(Value::as_i64).unwrap_or(0) + 1;
        by_tier.insert(tier_key, Value::from(count));
        if truthy(item.get("review_required")) {
            review_required += 1;
        }
    }
    let mut out = Map::new();
    out.insert("count".into(), Value::from(items.len() as i64));
    out.insert("by_tier".into(), Value::Object(by_tier));
    out.insert("review_required".into(), Value::from(review_required));
    Value::Object(out)
}

/// Mirrors `write_knowledge_doc`.
pub fn write_knowledge_doc(bundle_dir: &Path, item: &Value) -> Result<()> {
    let okf_path = item.get("okf_path").and_then(Value::as_str).unwrap_or_default();

    let mut front_matter = Map::new();
    for (key, default) in [
        ("type", Value::Null),
        ("title", Value::Null),
        ("review_status", Value::Null),
        ("lifecycle_status", Value::Null),
        ("confidence", Value::Null),
        ("knowledge_tier", Value::Null),
        ("review_required", Value::Null),
        ("evidence_status", Value::Null),
        ("code", Value::Null),
    ] {
        front_matter.insert(key.to_string(), item.get(key).cloned().unwrap_or(default));
    }
    front_matter.insert("knowledge_id".into(), item.get("id").cloned().unwrap_or(Value::Null));
    front_matter.insert("supersedes".into(), item.get("supersedes").cloned().unwrap_or(Value::Array(vec![])));
    front_matter.insert(
        "superseded_by".into(),
        item.get("superseded_by").cloned().unwrap_or(Value::Array(vec![])),
    );
    front_matter.insert("symbols".into(), item.get("symbols").cloned().unwrap_or(Value::Array(vec![])));
    front_matter.insert("relations".into(), item.get("relations").cloned().unwrap_or(Value::Array(vec![])));
    front_matter.insert("evidence".into(), item.get("evidence").cloned().unwrap_or(Value::Array(vec![])));

    let title = item.get("title").and_then(Value::as_str).unwrap_or("");
    let claim = item.get("claim").and_then(Value::as_str).unwrap_or("");
    let body = format!("# {title}\n\n{claim}\n");

    write_markdown(&bundle_dir.join(okf_path), &Value::Object(front_matter), &body)
}

/// Mirrors `author_neighborhood`.
pub fn author_neighborhood(relations: &[Value], symbol_by_id: &HashMap<String, Value>) -> Vec<Value> {
    let mut dedup: HashMap<String, Value> = HashMap::new();
    for relation in relations {
        if let Some(obj_id) = relation.get("object").and_then(Value::as_str) {
            if let Some(symbol) = symbol_by_id.get(obj_id) {
                dedup.insert(obj_id.to_string(), symbol.clone());
            }
        }
    }
    let mut result: Vec<Value> = dedup.into_values().collect();
    result.sort_by(|a, b| {
        let na = a.get("qualified_name").and_then(Value::as_str).unwrap_or("");
        let nb = b.get("qualified_name").and_then(Value::as_str).unwrap_or("");
        na.cmp(nb)
    });
    result
}

/// Mirrors `select_symbols_for_authoring`: pick the symbols worth spending
/// an LLM call on. Interpretation only pays off where there is enough
/// static signal to ground it, so symbols are gated on kind and on how
/// connected they are in the relation graph.
pub fn select_symbols_for_authoring(
    symbols: &[Value],
    relations_by_subject: &HashMap<String, Vec<Value>>,
    selection: &Value,
) -> Vec<Value> {
    let default_kinds: HashSet<String> = ["class", "function", "method"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let kinds: HashSet<String> = selection
        .get("kinds")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<HashSet<_>>())
        .filter(|s| !s.is_empty())
        .unwrap_or(default_kinds);
    let min_relations = selection.get("min_relations").and_then(Value::as_i64).unwrap_or(2);
    let include_tests = selection.get("include_tests").and_then(Value::as_bool).unwrap_or(false);
    let max_symbols = selection.get("max_symbols").and_then(Value::as_i64).unwrap_or(250);

    let empty: Vec<Value> = Vec::new();
    let mut selected: Vec<Value> = symbols
        .iter()
        .filter(|symbol| {
            let kind = symbol.get("kind").and_then(Value::as_str).unwrap_or("");
            let kind_ok = kinds.contains(kind) || (include_tests && kind == "test");
            let rel_count = relations_by_subject
                .get(symbol.get("id").and_then(Value::as_str).unwrap_or(""))
                .unwrap_or(&empty)
                .len() as i64;
            kind_ok && rel_count >= min_relations
        })
        .cloned()
        .collect();

    // Python: `key=lambda symbol: (-len(relations_by_subject.get(...)), symbol["qualified_name"])`
    // -- descending by relation count, ascending by qualified name.
    selected.sort_by(|a, b| {
        let ra = relations_by_subject
            .get(a.get("id").and_then(Value::as_str).unwrap_or(""))
            .map(|v| v.len())
            .unwrap_or(0);
        let rb = relations_by_subject
            .get(b.get("id").and_then(Value::as_str).unwrap_or(""))
            .map(|v| v.len())
            .unwrap_or(0);
        rb.cmp(&ra).then_with(|| {
            let na = a.get("qualified_name").and_then(Value::as_str).unwrap_or("");
            let nb = b.get("qualified_name").and_then(Value::as_str).unwrap_or("");
            na.cmp(nb)
        })
    });

    if max_symbols > 0 {
        selected.truncate(max_symbols as usize);
    }
    selected
}

/// Mirrors `authored_knowledge_item`: turn a grounded LLM candidate into an
/// OKF knowledge item. Anchored claims that only restate documented
/// behavior can sit at Tier 3. Everything inferential, and anything whose
/// quoted code could not be located in the symbol, is Tier 4 and
/// review-required.
pub fn authored_knowledge_item(symbol: &Value, claim: &Value, ordinal: i64, actor: &str, grounding: &Value) -> Value {
    let anchored = truthy(claim.get("anchored"));
    let claim_kind = claim.get("claim_kind").and_then(Value::as_str).unwrap_or("").to_string();
    let mut tier: i64 = if anchored && matches!(claim_kind.as_str(), "purpose" | "behavior") {
        3
    } else {
        4
    };
    if !anchored {
        let demote = grounding
            .get("demote_unanchored_to_tier")
            .and_then(value_as_int)
            .unwrap_or(4);
        tier = tier.max(demote);
    }
    let symbol_id = symbol.get("id").and_then(Value::as_str).unwrap_or("").to_string();
    let item_id = code_knowledge_id_for_claim(&symbol_id, &claim_kind, ordinal);
    let evidence_ids: Vec<Value> = symbol
        .get("evidence_id")
        .filter(|v| truthy(Some(v)))
        .map(|v| vec![v.clone()])
        .unwrap_or_default();

    let mut out = Map::new();
    out.insert("id".into(), Value::String(item_id.clone()));
    out.insert("type".into(), Value::String("Code Knowledge".into()));
    out.insert("title".into(), claim.get("title").cloned().unwrap_or(Value::Null));
    out.insert("claim".into(), claim.get("claim").cloned().unwrap_or(Value::Null));
    out.insert("knowledge_tier".into(), Value::from(tier));
    out.insert("review_status".into(), Value::String("proposed".into()));
    out.insert("lifecycle_status".into(), Value::String("active".into()));
    out.insert("confidence".into(), claim.get("confidence").cloned().unwrap_or(Value::Null));
    out.insert("review_required".into(), Value::Bool(tier >= 4));
    out.insert(
        "evidence_status".into(),
        Value::String(if !evidence_ids.is_empty() && anchored { "linked".into() } else { "llm_claimed".into() }),
    );
    out.insert("evidence".into(), Value::Array(evidence_ids));
    out.insert("relations".into(), sorted_unique_str_array(claim.get("relations")));
    out.insert("symbols".into(), sorted_unique_str_array(claim.get("symbols")));

    let mut code = Map::new();
    code.insert(
        "language".into(),
        symbol.get("language").cloned().unwrap_or_else(|| Value::String("python".into())),
    );
    code.insert("symbol_id".into(), Value::String(symbol_id));
    code.insert("qualified_name".into(), symbol.get("qualified_name").cloned().unwrap_or(Value::Null));
    code.insert("file".into(), symbol.get("file").cloned().unwrap_or(Value::Null));
    code.insert(
        "source_version_id".into(),
        symbol.get("source_version_id").cloned().unwrap_or(Value::Null),
    );
    code.insert("derivation".into(), Value::String("llm-authored".into()));
    code.insert("provider".into(), Value::String(actor.to_string()));
    code.insert("claim_kind".into(), Value::String(claim_kind));
    code.insert(
        "anchor_status".into(),
        Value::String(if anchored { "exact".into() } else { "llm_claimed".into() }),
    );
    code.insert("quoted_code".into(), claim.get("quoted_code").cloned().unwrap_or(Value::Null));
    out.insert("code".into(), Value::Object(code));

    out.insert("okf_path".into(), Value::String(format!("knowledge/{item_id}.md")));
    Value::Object(out)
}

struct PreparedRequest {
    symbol: Value,
    request: Value,
    key: String,
    cached_response: Option<Value>,
}

/// Bounded work-stealing dispatch over OS threads (no tokio/async): `workers`
/// threads pull indices off a shared queue and each computes `author(request)`
/// for its index, matching the Python original's `ThreadPoolExecutor` bound.
fn run_llm_calls(
    to_call: &[usize],
    requests: &[Value],
    author: &CodeAuthorFn,
    workers: usize,
) -> HashMap<usize, std::result::Result<Value, String>> {
    use std::sync::mpsc;
    use std::sync::Mutex;

    if to_call.is_empty() {
        return HashMap::new();
    }
    let worker_count = workers.max(1).min(to_call.len());
    let queue: Mutex<std::vec::IntoIter<usize>> = Mutex::new(to_call.to_vec().into_iter());
    let (tx, rx) = mpsc::channel();

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let tx = tx.clone();
            // `author: &CodeAuthorFn` is `&(dyn Fn(...) + Send + Sync)`: a
            // plain reference, `Copy` and `Send` (since the trait object it
            // points at is `Sync`), and `std::thread::scope` guarantees every
            // spawned thread below joins before this function returns -- so
            // capturing it by reference is sound without an `Arc::clone`
            // (there is no owning `Arc` in scope to clone here; the caller
            // owns the value this reference borrows from).
            let author = author;
            let queue = &queue;
            scope.spawn(move || loop {
                let next = queue.lock().expect("queue mutex poisoned").next();
                let Some(idx) = next else { break };
                let result = author(&requests[idx]).map_err(|e| e.to_string());
                let _ = tx.send((idx, result));
            });
        }
        drop(tx);
        let mut results = HashMap::new();
        for (idx, result) in rx {
            results.insert(idx, result);
        }
        results
    })
}

/// Mirrors `enrich_with_author`: run one LLM call per selected symbol and
/// keep only grounded claims.
///
/// Every candidate passes the shape, vocabulary, and anchor checks in
/// `validate_candidate` before it becomes a knowledge item. Rejections are
/// recorded rather than raised, so one bad response cannot fail a whole
/// mining run.
///
/// The actual network call is the only part of this that benefits from
/// concurrency (request building and cache lookups are fast and local), so
/// only that part runs on a bounded thread pool (`max_workers`, default from
/// `mining.max_workers` config, itself defaulting to 5) -- everything
/// before and after it, including every cache write, stays on the calling
/// thread, so nothing here needs its own locking.
pub fn enrich_with_author(
    bundle_dir: &Path,
    symbols: &[Value],
    relations_by_subject: &HashMap<String, Vec<Value>>,
    author: &CodeAuthorFn,
    actor: &str,
    max_workers: Option<i64>,
) -> Result<(Vec<Value>, Value)> {
    let selection = mining_selection(bundle_dir);
    let grounding = mining_grounding(bundle_dir);
    let workers = max_workers.unwrap_or_else(|| mining_max_workers(bundle_dir)).max(1) as usize;
    let candidates = select_symbols_for_authoring(symbols, relations_by_subject, &selection);
    let symbol_by_id: HashMap<String, Value> = symbols
        .iter()
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(|id| (id.to_string(), s.clone())))
        .collect();
    // WIRING: `EnrichmentCache` is defined in kl4a/codekb/cache.py (not this
    // batch). `new(bundle_dir, enabled)` / `.get(key)` / `.put(...)` /
    // `.prune(...)` / `.save()` / `.stats()` are inferred from call shape in
    // enrich_with_author's Python body; exact Rust signatures UNCONFIRMED.
    let mut cache = EnrichmentCache::new(bundle_dir, mining_cache_enabled(bundle_dir), crate::cache::CACHE_FILENAME);

    let empty_relations: Vec<Value> = Vec::new();
    let mut prepared: Vec<PreparedRequest> = Vec::with_capacity(candidates.len());
    for symbol in &candidates {
        let relations = relations_by_subject
            .get(symbol.get("id").and_then(Value::as_str).unwrap_or(""))
            .unwrap_or(&empty_relations);
        let neighborhood = author_neighborhood(relations, &symbol_by_id);
        let request = build_symbol_author_request(bundle_dir, symbol, relations, &neighborhood);
        let key = fingerprint(&request, actor, CODE_AUTHOR_SYSTEM_PROMPT);
        let cached_response = cache.get(&key);
        prepared.push(PreparedRequest {
            symbol: symbol.clone(),
            request,
            key,
            cached_response,
        });
    }
    let attempted = prepared.len() as i64;

    let to_call: Vec<usize> = prepared
        .iter()
        .enumerate()
        .filter(|(_, p)| p.cached_response.is_none())
        .map(|(idx, _)| idx)
        .collect();

    let mut called: i64 = 0;
    let mut call_responses: HashMap<usize, Value> = HashMap::new();
    let mut call_errors: HashMap<usize, String> = HashMap::new();

    if !to_call.is_empty() {
        eprintln!(
            "[mine] {}/{} symbols served from cache; making {} LLM call(s), up to {workers} at a time",
            prepared.len() - to_call.len(),
            prepared.len(),
            to_call.len(),
        );
        let requests: Vec<Value> = prepared.iter().map(|p| p.request.clone()).collect();
        let results = run_llm_calls(&to_call, &requests, author, workers);
        let total = to_call.len();
        for (done, idx) in to_call.iter().enumerate() {
            match results.get(idx) {
                Some(Ok(v)) => {
                    call_responses.insert(*idx, v.clone());
                }
                Some(Err(e)) => {
                    call_errors.insert(*idx, e.clone());
                }
                None => {
                    call_errors.insert(*idx, "author call produced no result".to_string());
                }
            }
            eprintln!("[mine] {}/{total} LLM call(s) completed", done + 1);
        }
    }

    let mut items: Vec<Value> = Vec::new();
    let mut rejected: Vec<Value> = Vec::new();

    let require_quoted_code = truthy(grounding.get("require_quoted_code"))
        || grounding.get("require_quoted_code").is_none();

    for (idx, entry) in prepared.iter().enumerate() {
        if let Some(err) = call_errors.get(&idx) {
            let mut rej = Map::new();
            rej.insert("symbol_id".into(), entry.symbol.get("id").cloned().unwrap_or(Value::Null));
            rej.insert("reason".into(), Value::String(format!("author call failed: {err}")));
            rejected.push(Value::Object(rej));
            continue;
        }

        let response: Value = if let Some(resp) = call_responses.get(&idx) {
            called += 1;
            // Only a usable response is worth remembering; a malformed one
            // should be retried rather than cached and re-parsed forever.
            if resp.is_object() {
                cache.put(
                    &entry.key,
                    resp.clone(),
                    entry.symbol.get("id").and_then(Value::as_str).unwrap_or(""),
                    entry.symbol.get("source_version_id").and_then(Value::as_str),
                );
            }
            resp.clone()
        } else {
            entry.cached_response.clone().unwrap_or(Value::Null)
        };

        let mut seen_kinds: HashMap<String, i64> = HashMap::new();
        let empty_items: Vec<Value> = Vec::new();
        let knowledge_items = response
            .get("knowledge_items")
            .and_then(Value::as_array)
            .unwrap_or(&empty_items);
        for candidate in knowledge_items {
            match validate_candidate(candidate, &entry.request, require_quoted_code) {
                Ok(claim) => {
                    let claim_kind = claim.get("claim_kind").and_then(Value::as_str).unwrap_or("").to_string();
                    let ordinal = *seen_kinds.get(&claim_kind).unwrap_or(&0);
                    seen_kinds.insert(claim_kind, ordinal + 1);
                    items.push(authored_knowledge_item(&entry.symbol, &claim, ordinal, actor, &grounding));
                }
                Err(reason) => {
                    let mut rej = Map::new();
                    rej.insert("symbol_id".into(), entry.symbol.get("id").cloned().unwrap_or(Value::Null));
                    rej.insert("reason".into(), Value::String(reason.to_string()));
                    rejected.push(Value::Object(rej));
                }
            }
        }
    }

    let all_symbol_ids: HashSet<String> = symbols
        .iter()
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(|s| s.to_string()))
        .collect();
    cache.prune(&all_symbol_ids);
    cache.save()?;

    eprintln!(
        "[mine] done: {} claim(s) accepted from {attempted} symbol(s), {called} live call(s), {} rejected",
        items.len(),
        rejected.len(),
    );

    let mut enrichment = Map::new();
    enrichment.insert("attempted".into(), Value::from(attempted));
    enrichment.insert("accepted".into(), Value::from(items.len() as i64));
    enrichment.insert("rejected".into(), Value::Array(rejected));
    enrichment.insert("provider_calls".into(), Value::from(called));
    enrichment.insert(
        "cache".into(),
        serde_json::to_value(cache.stats()).unwrap_or(Value::Null),
    );

    Ok((items, Value::Object(enrichment)))
}

fn default_enrichment() -> Value {
    let mut cache_stats = Map::new();
    cache_stats.insert("enabled".into(), Value::Bool(false));
    cache_stats.insert("hits".into(), Value::from(0));
    cache_stats.insert("misses".into(), Value::from(0));
    cache_stats.insert("pruned".into(), Value::from(0));
    cache_stats.insert("entries".into(), Value::from(0));

    let mut out = Map::new();
    out.insert("attempted".into(), Value::from(0));
    out.insert("accepted".into(), Value::from(0));
    out.insert("rejected".into(), Value::Array(vec![]));
    out.insert("provider_calls".into(), Value::from(0));
    out.insert("cache".into(), Value::Object(cache_stats));
    Value::Object(out)
}

fn lifecycle_summary_to_value(items: &[Value]) -> Value {
    let summary = crate::lifecycle::lifecycle_summary(items);
    let mut out = Map::new();
    for (status, count) in summary {
        out.insert(status, Value::from(count));
    }
    Value::Object(out)
}

/// Mirrors `mine_code_bundle`: mine reviewable code knowledge from the
/// static graph.
///
/// `provider` selects the mode. Static providers run deterministic
/// extraction only. Hybrid providers run that same extraction first and
/// then add an LLM enrichment pass on top of it, so a failed or skipped LLM
/// call always degrades to a valid static bundle rather than to no bundle.
pub fn mine_code_bundle(
    bundle_dir: &Path,
    provider: &str,
    author: Option<&CodeAuthorFn>,
    procedure_author: Option<&CodeAuthorFn>,
) -> Result<Value> {
    if !STATIC_PROVIDERS.contains(provider) && !HYBRID_PROVIDERS.contains(provider) {
        anyhow::bail!("unsupported code mining provider: {provider}");
    }

    let mut default_symbols_state = Map::new();
    default_symbols_state.insert("symbols".into(), Value::Array(vec![]));
    let mut default_relations_state = Map::new();
    default_relations_state.insert("relations".into(), Value::Array(vec![]));
    let mut default_items_state = Map::new();
    default_items_state.insert("items".into(), Value::Array(vec![]));

    let symbols_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_symbols.json"),
        Value::Object(default_symbols_state),
    );
    let relations_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_relations.json"),
        Value::Object(default_relations_state),
    );
    let previous = read_json(
        &bundle_dir.join(STATE_DIR).join("code_knowledge.json"),
        Value::Object(default_items_state),
    );

    let mut symbols: Vec<Value> = symbols_state
        .get("symbols")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    symbols.sort_by(|a, b| {
        let ka = a.get("qualified_name").and_then(Value::as_str).unwrap_or("");
        let kb = b.get("qualified_name").and_then(Value::as_str).unwrap_or("");
        ka.cmp(kb)
    });

    let relations: Vec<Value> = relations_state
        .get("relations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let relations_by_subject = group_relations_by_subject(&relations);
    let symbol_by_id: HashMap<String, Value> = symbols
        .iter()
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(|id| (id.to_string(), s.clone())))
        .collect();

    let empty_relations: Vec<Value> = Vec::new();
    let mut items: Vec<Value> = Vec::new();
    for symbol in &symbols {
        let relations = relations_by_subject
            .get(symbol.get("id").and_then(Value::as_str).unwrap_or(""))
            .unwrap_or(&empty_relations);
        items.extend(symbol_claims(symbol, relations));
    }

    // Clustering + the mechanical procedure claim are pure static-graph
    // analysis, no LLM involved, so they run (and produce claims) regardless
    // of provider -- the narrative pass below is the only part that needs
    // hybrid mode.
    let mut clusters: Vec<Value> = Vec::new();
    if procedures_enabled(bundle_dir) {
        clusters = cluster_symbols(&symbols, &relations_by_subject, procedures_min_calls(bundle_dir));
        items.extend(static_procedure_claims(&clusters, &symbol_by_id));
    }
    items = dedupe_items(items);

    // Same shape in static mode, so readers need no special case.
    let mut enrichment: Value = default_enrichment();

    if HYBRID_PROVIDERS.contains(provider) {
        let grounding = mining_grounding(bundle_dir);
        let default_author: Arc<CodeAuthorFn> = Arc::new(azure_code_author);
        let author_fn = author.unwrap_or_else(|| default_author.as_ref());
        let actor = format!("codekb/{provider}");
        let (llm_items, enrichment_result) =
            enrich_with_author(bundle_dir, &symbols, &relations_by_subject, author_fn, &actor, None)?;
        items = dedupe_items(items.into_iter().chain(llm_items).collect());
        enrichment = enrichment_result;

        if !clusters.is_empty() {
            let default_proc_author: Arc<CodeAuthorFn> = Arc::new(azure_procedure_author);
            let proc_author_fn = procedure_author.unwrap_or_else(|| default_proc_author.as_ref());
            let (procedure_items, procedure_enrichment) = enrich_with_procedure_author(
                bundle_dir,
                &clusters,
                &symbol_by_id,
                &relations_by_subject,
                proc_author_fn,
                &actor,
                &grounding,
                mining_max_workers(bundle_dir).max(1) as usize,
                mining_cache_enabled(bundle_dir),
            )?;
            items = dedupe_items(items.into_iter().chain(procedure_items).collect());
            if let Some(obj) = enrichment.as_object_mut() {
                obj.insert("procedures".to_string(), procedure_enrichment);
            }
        }
    }

    let previous_items: Vec<Value> = previous.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
    items = merge_code_items(bundle_dir, &previous_items, &items, &symbols)?;
    for item in &items {
        write_knowledge_doc(bundle_dir, item)?;
    }

    let mut result = Map::new();
    result.insert("items".into(), Value::Array(items.clone()));
    result.insert("provider".into(), Value::String(provider.to_string()));
    result.insert("summary".into(), knowledge_summary(&items));
    result.insert("lifecycle".into(), lifecycle_summary_to_value(&items));
    result.insert("enrichment".into(), enrichment);

    let result_value = Value::Object(result);
    write_code_state(bundle_dir, "code_knowledge.json", &result_value)?;
    Ok(result_value)
}

#[cfg(test)]
mod value_as_int_tests {
    use super::value_as_int;
    use serde_json::json;

    /// Python's `int()` accepts an integer-valued float (truncating toward
    /// zero) but rejects a decimal *string* outright (`ValueError`), which
    /// both `mining_max_workers` and the `demote_unanchored_to_tier` lookup
    /// catch and fall back on. Matches the fix to `value_as_int`.
    #[test]
    fn matches_python_int_coercion() {
        assert_eq!(value_as_int(&json!(3)), Some(3));
        assert_eq!(value_as_int(&json!(3.9)), Some(3)); // int(3.9) == 3
        assert_eq!(value_as_int(&json!("3")), Some(3)); // int("3") == 3
        assert_eq!(value_as_int(&json!("  3  ")), Some(3)); // int("  3  ") == 3
        assert_eq!(value_as_int(&json!("+5")), Some(5)); // int("+5") == 5
        assert_eq!(value_as_int(&json!(true)), Some(1)); // int(True) == 1
        assert_eq!(value_as_int(&json!("3.5")), None); // int("3.5") raises ValueError
        assert_eq!(value_as_int(&json!("abc")), None); // int("abc") raises ValueError
        assert_eq!(value_as_int(&json!(null)), None); // int(None) raises TypeError
    }
}
