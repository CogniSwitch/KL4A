//! Port of `kl4a/codekb/procedures.py`.
//!
//! Multi-symbol "procedure" knowledge claims: a deterministic, static-graph
//! clustering of an entry function/method with the same-file helpers it
//! calls (no LLM required — [`static_procedure_claims`]), plus an optional
//! LLM-authored narrative pass ([`enrich_with_procedure_author`]) that only
//! keeps claims grounded against the cluster's own members. Ported
//! symbol-for-symbol from the Python source (verified via tools-code MCP
//! `code_symbols_get`).
//!
//! ## Cross-batch dependencies (NOT part of this batch — report only)
//!
//! - `crate::ids::{bounded_id, code_knowledge_id_for, code_procedure_id_for}`
//! - `crate::state::STATE_DIR`, `crate::bundle_store::load_manifest`
//! - `crate::cache::{EnrichmentCache, fingerprint}` — `EnrichmentCache::new(bundle_dir,
//!   enabled, filename) -> EnrichmentCache`, with `.get(key) -> Option<Value>`,
//!   `.put(key, response, symbol_id, source_version_id: Option<&str>)`,
//!   `.prune(&HashSet<String>)`, `.save() -> Result<()>`, `.stats() -> Value`.
//!   `fingerprint(request: &Value, actor: &str, prompt: &str) -> String`.
//! - `crate::author::{ClaimRejected, require_string, require_id_list,
//!   clamp_confidence, read_symbol_source}` — `ClaimRejected` is the distinct
//!   validation-failure error type `enrich_with_procedure_author` catches
//!   specifically (separately from an author-call/provider failure), so
//!   [`validate_procedure_candidate`] returns `Result<_, ClaimRejected>`
//!   rather than `anyhow::Result`, matching the shared convention of
//!   `thiserror` for a genuinely distinct exception kind.
//! - `crate::llm_settings::complete(messages: &[Value]) -> Result<String>`
//!   and `crate::llm_provider::parse_author_response(text: &str) ->
//!   Result<Value>` (used by [`azure_procedure_author`]).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use serde_json::{json, Map, Value};

use crate::author::{clamp_confidence, read_symbol_source, require_id_list, require_string, ClaimRejected, CodeAuthorFn};
use crate::bundle_store::load_manifest;
use crate::cache::{fingerprint, EnrichmentCache};
use crate::ids::{bounded_id, code_knowledge_id_for, code_procedure_id_for};
use crate::llm_provider::parse_author_response;
use crate::llm_settings;

/// Port of `kl4a.codekb.procedures.PROCEDURE_CACHE_FILENAME`.
pub const PROCEDURE_CACHE_FILENAME: &str = "code_enrichment_procedures.json";

/// Port of `kl4a.codekb.procedures.PROCEDURE_AUTHOR_SYSTEM_PROMPT`.
pub const PROCEDURE_AUTHOR_SYSTEM_PROMPT: &str = "You are an OKF v0.2 author for source-code knowledge.\nReturn only a JSON object, with no Markdown fences and no commentary.\n\nThe JSON object must have:\n- knowledge_items: list of claim objects (usually exactly one)\n\nEvery knowledge_items entry must include:\n  claim_kind: must be exactly \"procedure\"\n  title: short noun phrase naming the overall procedure\n  claim: a SINGLE STRING (never a list/array) containing a numbered-step,\n    one-sentence-per-step description of what the whole sequence of provided\n    members accomplishes together, in plain language - write the steps as\n    \"1. ... 2. ... 3. ...\" inline within that one string, the same way you\n    would write a numbered list in prose\n  symbols: list of symbol ids - must include every id in required_member_ids,\n    each copied verbatim from allowed_symbol_ids\n  relations: list of relation ids, each copied from allowed_relation_ids\n  quoted_code: an exact substring copied character-for-character from any one\n    member's source\n  confidence: number between 0 and 1\n\nHard rules:\n- You may only cite ids that appear in allowed_symbol_ids and allowed_relation_ids.\n  Never invent, guess, complete, or normalize an id.\n- symbols must include every id in required_member_ids - this claim is about the\n  whole procedure, not a subset of it.\n- quoted_code must be copied verbatim from one of the provided members' source.\n  Do not reformat, re-indent, elide with ellipses, or paraphrase it.\n- Describe only what the provided members and relations show. Do not describe\n  code you were not given, and do not assume a step exists because a name\n  suggests it.\n- If the members do not actually form a coherent procedure, return an empty\n  knowledge_items list. An empty list is a valid and useful answer.\n- Never state that a claim is approved, verified, or safe to rely on.\n";

fn vstr(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}
fn vid(v: &Value) -> String {
    vstr(v, "id")
}

/// Port of `kl4a.codekb.procedures.cluster_symbols`.
///
/// Deterministic, static-graph-only grouping — no LLM involved, so it runs
/// (and can produce claims) even under fully static mining. A symbol only
/// ever anchors one cluster (its own direct same-file callees); member
/// order is the callees' definition order (`line_start`), not confirmed
/// call-site/runtime order.
pub fn cluster_symbols(symbols: &[Value], relations_by_subject: &HashMap<String, Vec<Value>>, min_calls: usize) -> Vec<Value> {
    let symbol_by_id: HashMap<String, Value> = symbols.iter().map(|s| (vid(s), s.clone())).collect();
    let mut clusters = Vec::new();
    for symbol in symbols {
        let kind = vstr(symbol, "kind");
        if kind != "function" && kind != "method" {
            continue;
        }
        let symbol_id = vid(symbol);
        let relations = relations_by_subject.get(&symbol_id).cloned().unwrap_or_default();
        let call_relations: Vec<&Value> = relations
            .iter()
            .filter(|r| {
                vstr(r, "predicate") == "calls"
                    && vstr(r, "object_kind") == "symbol"
                    && r.get("relation").and_then(|rel| rel.get("resolution_status")).and_then(Value::as_str) == Some("exact")
                    && symbol_by_id.contains_key(&vstr(r, "object"))
                    && vstr(r, "object") != symbol_id
            })
            .collect();

        let mut members_by_id: HashMap<String, Value> = HashMap::new();
        for relation in &call_relations {
            let callee_id = vstr(relation, "object");
            let callee = &symbol_by_id[&callee_id];
            let ckind = vstr(callee, "kind");
            if ckind != "function" && ckind != "method" {
                continue;
            }
            if vstr(callee, "file") != vstr(symbol, "file") {
                continue;
            }
            members_by_id.insert(callee_id, callee.clone());
        }
        if members_by_id.len() < min_calls {
            continue;
        }
        let mut members: Vec<Value> = members_by_id.values().cloned().collect();
        members.sort_by(|a, b| {
            let la = a.get("line_start").and_then(Value::as_i64).unwrap_or(0);
            let lb = b.get("line_start").and_then(Value::as_i64).unwrap_or(0);
            la.cmp(&lb).then_with(|| vstr(a, "qualified_name").cmp(&vstr(b, "qualified_name")))
        });
        let mut member_ids = vec![symbol_id.clone()];
        member_ids.extend(members.iter().map(vid));
        let mut relation_ids: Vec<String> = call_relations
            .iter()
            .filter(|r| members_by_id.contains_key(&vstr(r, "object")))
            .map(|r| vid(r))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        relation_ids.sort();
        clusters.push(json!({
            "id": code_procedure_id_for(&symbol_id),
            "entry_symbol_id": symbol_id,
            "member_ids": member_ids,
            "relation_ids": relation_ids,
        }));
    }
    clusters
}

/// Port of `kl4a.codekb.procedures.static_procedure_claims`.
///
/// Mechanical, deterministic procedure claims — no LLM required. Mirrors
/// the shape of `codekb.knowledge.knowledge_item`'s output, but a procedure
/// spans multiple symbols so it cannot reuse that single-symbol helper.
pub fn static_procedure_claims(clusters: &[Value], symbol_by_id: &HashMap<String, Value>) -> Vec<Value> {
    let mut claims = Vec::new();
    for cluster in clusters {
        let entry_id = vstr(cluster, "entry_symbol_id");
        let Some(entry) = symbol_by_id.get(&entry_id) else { continue };
        let member_ids: Vec<String> = cluster
            .get("member_ids")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let members: Vec<&Value> = member_ids.iter().filter_map(|id| symbol_by_id.get(id)).collect();
        if members.len() < 2 {
            continue;
        }
        let steps: Vec<String> = members
            .iter()
            .enumerate()
            .map(|(i, m)| format!("{}. {}", i + 1, vstr(m, "qualified_name")))
            .collect();
        let claim_text = format!(
            "{} calls {} same-file helper(s), forming a {}-step sequence: {}. Step order reflects where each helper is defined in the file, not confirmed call-site or runtime order.",
            vstr(entry, "qualified_name"),
            members.len() - 1,
            members.len(),
            steps.join("; "),
        );
        let item_id = code_knowledge_id_for(&vstr(cluster, "id"), "procedure");
        let evidence_ids: Vec<Value> = match entry.get("evidence_id").and_then(Value::as_str) {
            Some(e) => vec![json!(e)],
            None => vec![],
        };
        let mut member_id_set: Vec<String> = members.iter().map(|m| vid(m)).collect::<HashSet<_>>().into_iter().collect();
        member_id_set.sort();
        claims.push(json!({
            "id": item_id,
            "type": "Code Knowledge",
            "title": format!("{} is part of a multi-step procedure", vstr(entry, "qualified_name")),
            "claim": claim_text,
            "knowledge_tier": 3,
            "review_status": "proposed",
            "lifecycle_status": "active",
            "confidence": 0.7,
            "review_required": false,
            "evidence_status": if evidence_ids.is_empty() { "unresolved" } else { "linked" },
            "evidence": evidence_ids,
            "relations": cluster.get("relation_ids").cloned().unwrap_or_else(|| json!([])),
            "symbols": member_id_set.clone(),
            "code": {
                "language": entry.get("language").and_then(Value::as_str).unwrap_or("python"),
                "symbol_id": vid(entry),
                "qualified_name": vstr(entry, "qualified_name"),
                "file": entry.get("file").cloned().unwrap_or(Value::Null),
                "source_version_id": entry.get("source_version_id").cloned().unwrap_or(Value::Null),
                "derivation": "fixture-procedure-cluster",
                "provider": "codekb/static",
                "claim_kind": "procedure",
                "anchor_status": "exact",
                "member_symbol_ids": member_id_set.clone(),
            },
            "okf_path": format!("knowledge/{item_id}.md"),
        }));
    }
    claims
}

/// Port of `kl4a.codekb.procedures.validate_procedure_candidate`.
///
/// Grounding layers for a procedure candidate, mirroring
/// `author.validate_candidate`. Unlike the per-symbol version: every
/// required cluster member id must be cited, and the quoted code anchors
/// against any one member's source rather than a single symbol's.
pub fn validate_procedure_candidate(
    candidate: &Value,
    request: &Value,
    require_quoted_code: bool,
) -> Result<Map<String, Value>, ClaimRejected> {
    if !candidate.is_object() {
        return Err(ClaimRejected::new("candidate is not an object"));
    }
    let candidate_obj = candidate;

    let claim_kind = require_string(candidate_obj, "claim_kind")?;
    if claim_kind != "procedure" {
        return Err(ClaimRejected::new(format!("unknown claim_kind: {claim_kind}")));
    }
    let title = require_string(candidate_obj, "title")?;
    let claim = require_string(candidate_obj, "claim")?;

    let allowed_symbols: HashSet<String> = request
        .get("allowed_symbol_ids")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let allowed_relations: HashSet<String> = request
        .get("allowed_relation_ids")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let symbols = require_id_list(candidate_obj, "symbols", false)?;
    let relations = require_id_list(candidate_obj, "relations", true)?;

    let invented_symbols: Vec<&String> = symbols.iter().filter(|s| !allowed_symbols.contains(*s)).collect();
    if !invented_symbols.is_empty() {
        return Err(ClaimRejected::new(format!(
            "cites symbol id(s) outside allowed vocabulary: {}",
            invented_symbols.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        )));
    }
    let invented_relations: Vec<&String> = relations.iter().filter(|r| !allowed_relations.contains(*r)).collect();
    if !invented_relations.is_empty() {
        return Err(ClaimRejected::new(format!(
            "cites relation id(s) outside allowed vocabulary: {}",
            invented_relations.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        )));
    }
    let required: HashSet<String> = request
        .get("required_member_ids")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let symbols_set: HashSet<String> = symbols.iter().cloned().collect();
    let mut missing_members: Vec<&String> = required.iter().filter(|m| !symbols_set.contains(*m)).collect();
    if !missing_members.is_empty() {
        missing_members.sort();
        return Err(ClaimRejected::new(format!(
            "claim omits required cluster member id(s): {}",
            missing_members.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        )));
    }

    let quoted_code = candidate_obj.get("quoted_code").and_then(Value::as_str).unwrap_or("").to_string();
    if require_quoted_code && quoted_code.trim().is_empty() {
        return Err(ClaimRejected::new("missing quoted_code"));
    }
    let anchored = !quoted_code.trim().is_empty()
        && request
            .get("members")
            .and_then(Value::as_array)
            .map(|members| members.iter().any(|m| m.get("source").and_then(Value::as_str).map(|s| s.contains(&quoted_code)).unwrap_or(false)))
            .unwrap_or(false);

    let mut result = Map::new();
    result.insert("claim_kind".into(), json!(claim_kind));
    result.insert("title".into(), json!(title));
    result.insert("claim".into(), json!(claim));
    result.insert("symbols".into(), json!(symbols));
    result.insert("relations".into(), json!(relations));
    result.insert("quoted_code".into(), json!(quoted_code));
    result.insert("anchored".into(), json!(anchored));
    result.insert("confidence".into(), json!(clamp_confidence(candidate_obj.get("confidence"))));
    Ok(result)
}

/// Port of `kl4a.codekb.procedures.code_knowledge_id_for_procedure_claim`.
pub fn code_knowledge_id_for_procedure_claim(cluster_id: &str, ordinal: usize) -> String {
    let suffix = if ordinal == 0 { "llm-procedure".to_string() } else { format!("llm-procedure-{}", ordinal + 1) };
    bounded_id("ki", &format!("{cluster_id}-{suffix}"), 120)
}

/// Port of `kl4a.codekb.procedures.authored_procedure_item`.
///
/// Turns a grounded procedure candidate into an OKF knowledge item. A
/// procedure claim narrates how several symbols work together, a bigger
/// inferential leap than a single-symbol claim, so unlike
/// `authored_knowledge_item` this never grants Tier 3 — it always lands at
/// Tier 4 (review-required), the same conservative treatment the existing
/// multi-inference claim kinds already get.
pub fn authored_procedure_item(
    cluster: &Value,
    claim: &Map<String, Value>,
    ordinal: usize,
    actor: &str,
    grounding: &Value,
    symbol_by_id: &HashMap<String, Value>,
) -> Value {
    let entry = &symbol_by_id[&vstr(cluster, "entry_symbol_id")];
    let anchored = claim.get("anchored").and_then(Value::as_bool).unwrap_or(false);
    let tier = grounding.get("demote_unanchored_to_tier").and_then(Value::as_i64).unwrap_or(4).max(4);
    let item_id = code_knowledge_id_for_procedure_claim(&vstr(cluster, "id"), ordinal);
    let evidence_ids: Vec<Value> = match entry.get("evidence_id").and_then(Value::as_str) {
        Some(e) => vec![json!(e)],
        None => vec![],
    };
    let claim_symbols: HashSet<String> = claim
        .get("symbols")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let cluster_members: HashSet<String> = cluster
        .get("member_ids")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let mut member_ids: Vec<String> = claim_symbols.union(&cluster_members).cloned().collect();
    member_ids.sort();
    let mut relations: Vec<Value> = claim
        .get("relations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    relations.sort_by_key(|v| v.as_str().unwrap_or("").to_string());
    let quoted_code = claim.get("quoted_code").and_then(Value::as_str).unwrap_or("").to_string();

    json!({
        "id": item_id,
        "type": "Code Knowledge",
        "title": claim.get("title").cloned().unwrap_or(Value::Null),
        "claim": claim.get("claim").cloned().unwrap_or(Value::Null),
        "knowledge_tier": tier,
        "review_status": "proposed",
        "lifecycle_status": "active",
        "confidence": claim.get("confidence").cloned().unwrap_or(json!(0.0)),
        "review_required": tier >= 4,
        "evidence_status": if !evidence_ids.is_empty() && anchored { "linked" } else { "llm_claimed" },
        "evidence": evidence_ids,
        "relations": relations,
        "symbols": member_ids.clone(),
        "code": {
            "language": entry.get("language").and_then(Value::as_str).unwrap_or("python"),
            "symbol_id": vid(entry),
            "qualified_name": vstr(entry, "qualified_name"),
            "file": entry.get("file").cloned().unwrap_or(Value::Null),
            "source_version_id": entry.get("source_version_id").cloned().unwrap_or(Value::Null),
            "derivation": "llm-authored",
            "provider": actor,
            "claim_kind": "procedure",
            "anchor_status": if anchored { "exact" } else { "llm_claimed" },
            "quoted_code": quoted_code,
            "member_symbol_ids": member_ids,
        },
        "okf_path": format!("knowledge/{item_id}.md"),
    })
}

/// Port of `kl4a.codekb.procedures.build_procedure_author_request`.
///
/// Builds the LLM request from bundle state only, mirroring the per-symbol
/// builder.
pub fn build_procedure_author_request(
    bundle_dir: &Path,
    cluster: &Value,
    symbol_by_id: &HashMap<String, Value>,
    relations_by_subject: &HashMap<String, Vec<Value>>,
) -> Result<Value> {
    let member_ids: Vec<String> = cluster
        .get("member_ids")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).filter(|id| symbol_by_id.contains_key(id)).collect())
        .unwrap_or_default();
    let members: Vec<&Value> = member_ids.iter().map(|id| &symbol_by_id[id]).collect();
    let mut relation_ids: HashSet<String> = cluster
        .get("relation_ids")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    for member in &members {
        for relation in relations_by_subject.get(&vid(member)).into_iter().flatten() {
            relation_ids.insert(vid(relation));
        }
    }
    let mut member_ids_sorted = member_ids.clone();
    member_ids_sorted.sort();
    let mut relation_ids_sorted: Vec<String> = relation_ids.into_iter().collect();
    relation_ids_sorted.sort();

    let mut members_json = Vec::new();
    for member in &members {
        members_json.push(json!({
            "id": vid(member),
            "qualified_name": vstr(member, "qualified_name"),
            "kind": member.get("kind").cloned().unwrap_or(Value::Null),
            "language": member.get("language").and_then(Value::as_str).unwrap_or("python"),
            "file": member.get("file").cloned().unwrap_or(Value::Null),
            "signature": member.get("signature").cloned().unwrap_or(Value::Null),
            "docstring": member.get("docstring").and_then(Value::as_str).unwrap_or(""),
            "line_start": member.get("line_start").cloned().unwrap_or(Value::Null),
            "line_end": member.get("line_end").cloned().unwrap_or(Value::Null),
            "source": read_symbol_source(bundle_dir, member),
        }));
    }

    Ok(json!({
        "instruction": "Author one OKF 'procedure' code knowledge claim describing what these members do together, as a numbered sequence. Cite only the supplied ids and quote only the supplied source. Return JSON with knowledge_items.",
        "required_member_ids": member_ids_sorted.clone(),
        "members": members_json,
        "allowed_symbol_ids": member_ids_sorted,
        "allowed_relation_ids": relation_ids_sorted,
    }))
}

/// Port of `kl4a.codekb.procedures.build_procedure_author_messages`.
pub fn build_procedure_author_messages(request: &Value) -> Vec<Value> {
    vec![
        json!({"role": "system", "content": PROCEDURE_AUTHOR_SYSTEM_PROMPT}),
        json!({
            "role": "user",
            "content": format!(
                "Author one OKF 'procedure' code knowledge claim for these members. Return only JSON matching the required shape.\n\n{}",
                to_sorted_pretty_json(request)
            ),
        }),
    ]
}

fn to_sorted_pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(&sort_keys(value)).unwrap_or_default()
}

fn sort_keys(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = Map::new();
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for key in keys {
                sorted.insert(key.clone(), sort_keys(&map[key]));
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(sort_keys).collect()),
        other => other.clone(),
    }
}

/// Port of `kl4a.codekb.procedures.azure_procedure_author`.
pub fn azure_procedure_author(request: &Value) -> Result<Value> {
    let text = llm_settings::complete(&build_procedure_author_messages(request), None)?;
    parse_author_response(&text)
}

/// Port of `kl4a.codekb.procedures.procedures_config`.
pub fn procedures_config(bundle_dir: &Path) -> Value {
    let Ok(manifest) = load_manifest(bundle_dir) else { return json!({}) };
    manifest
        .get("codekb")
        .and_then(|c| c.get("mining"))
        .and_then(|m| m.get("procedures"))
        .cloned()
        .unwrap_or_else(|| json!({}))
}

/// Port of `kl4a.codekb.procedures.procedures_enabled`.
///
/// Whether clustering + procedure claims run at all. Defaults to on.
pub fn procedures_enabled(bundle_dir: &Path) -> bool {
    procedures_config(bundle_dir).get("enabled").and_then(Value::as_bool).unwrap_or(true)
}

/// Port of `kl4a.codekb.procedures.procedures_min_calls`.
///
/// Minimum same-file resolved calls an entry symbol needs to anchor a
/// cluster.
pub fn procedures_min_calls(bundle_dir: &Path) -> usize {
    procedures_config(bundle_dir)
        .get("min_calls")
        .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .map(|v| (v as usize).max(2))
        .unwrap_or(2)
}

/// Port of `kl4a.codekb.procedures.enrich_with_procedure_author`.
///
/// Runs one LLM call per candidate cluster and keeps only grounded
/// procedure claims. Mirrors `codekb.knowledge.enrich_with_author`'s
/// cache/concurrency shape against its own cache file
/// (`PROCEDURE_CACHE_FILENAME`) so pruning this pass can never evict the
/// per-symbol pass's entries or vice versa.
///
/// Python uses a `ThreadPoolExecutor(max_workers=workers)`; there is no
/// thread-pool crate in this workspace's dependency list, so concurrency is
/// approximated here with `std::thread::scope` (bounded to `workers`
/// in-flight threads at a time via chunking) — behaviorally equivalent
/// (same bounded parallelism, same "wait for this batch, print progress"
/// shape) but not the exact same scheduling primitive.
pub fn enrich_with_procedure_author(
    bundle_dir: &Path,
    clusters: &[Value],
    symbol_by_id: &HashMap<String, Value>,
    relations_by_subject: &HashMap<String, Vec<Value>>,
    author: &CodeAuthorFn,
    actor: &str,
    grounding: &Value,
    max_workers: usize,
    cache_enabled: bool,
) -> Result<(Vec<Value>, Value)> {
    let workers = max_workers.max(1);
    let mut cache = EnrichmentCache::new(bundle_dir, cache_enabled, PROCEDURE_CACHE_FILENAME);

    let mut items: Vec<Value> = Vec::new();
    let mut rejected: Vec<Value> = Vec::new();
    let mut attempted = 0usize;
    let mut called = 0usize;

    struct Prepared {
        cluster: Value,
        request: Value,
        key: String,
        cached_response: Option<Value>,
    }

    let mut prepared: Vec<Prepared> = Vec::new();
    for cluster in clusters {
        let request = build_procedure_author_request(bundle_dir, cluster, symbol_by_id, relations_by_subject)?;
        attempted += 1;
        let key = fingerprint(&request, actor, PROCEDURE_AUTHOR_SYSTEM_PROMPT);
        let cached_response = cache.get(&key);
        prepared.push(Prepared { cluster: cluster.clone(), request, key, cached_response });
    }

    let to_call_idx: Vec<usize> = prepared.iter().enumerate().filter(|(_, p)| p.cached_response.is_none()).map(|(i, _)| i).collect();
    let mut call_responses: HashMap<usize, Value> = HashMap::new();
    let mut call_errors: HashMap<usize, String> = HashMap::new();

    if !to_call_idx.is_empty() {
        eprintln!(
            "[mine:procedures] {}/{} cluster(s) served from cache; making {} LLM call(s), up to {workers} at a time",
            prepared.len() - to_call_idx.len(),
            prepared.len(),
            to_call_idx.len(),
        );
        let mut done = 0usize;
        for chunk in to_call_idx.chunks(workers) {
            std::thread::scope(|scope| {
                let handles: Vec<_> = chunk
                    .iter()
                    .map(|&idx| {
                        let request = &prepared[idx].request;
                        scope.spawn(move || (idx, author(request)))
                    })
                    .collect();
                for handle in handles {
                    let (idx, result) = handle.join().unwrap_or_else(|_| (0, Err(anyhow::anyhow!("author call panicked"))));
                    match result {
                        Ok(v) => {
                            call_responses.insert(idx, v);
                        }
                        Err(e) => {
                            call_errors.insert(idx, e.to_string());
                        }
                    }
                    done += 1;
                    eprintln!("[mine:procedures] {done}/{} LLM call(s) completed", to_call_idx.len());
                }
            });
        }
    }

    for (i, entry) in prepared.iter().enumerate() {
        let cluster = &entry.cluster;
        let cluster_id = vstr(cluster, "id");
        if let Some(err) = call_errors.get(&i) {
            rejected.push(json!({"cluster_id": cluster_id, "reason": format!("author call failed: {err}")}));
            continue;
        }
        let response = if let Some(response) = call_responses.get(&i) {
            called += 1;
            if response.is_object() {
                cache.put(&entry.key, response.clone(), &cluster_id, None);
            }
            response.clone()
        } else {
            entry.cached_response.clone().unwrap_or(Value::Null)
        };

        let knowledge_items = response.get("knowledge_items").and_then(Value::as_array).cloned().unwrap_or_default();
        for (ordinal, candidate) in knowledge_items.into_iter().enumerate() {
            let require_quoted = grounding.get("require_quoted_code").and_then(Value::as_bool).unwrap_or(true);
            match validate_procedure_candidate(&candidate, &entry.request, require_quoted) {
                Ok(claim) => {
                    items.push(authored_procedure_item(cluster, &claim, ordinal, actor, grounding, symbol_by_id));
                }
                Err(exc) => {
                    rejected.push(json!({"cluster_id": cluster_id, "reason": exc.to_string()}));
                }
            }
        }
    }

    let known_ids: HashSet<String> = clusters.iter().map(|c| vstr(c, "id")).collect();
    cache.prune(&known_ids);
    cache.save()?;

    eprintln!(
        "[mine:procedures] done: {} claim(s) accepted from {attempted} cluster(s), {called} live call(s), {} rejected",
        items.len(),
        rejected.len(),
    );

    let summary = json!({
        "attempted": attempted,
        "accepted": items.len(),
        "rejected": rejected,
        "provider_calls": called,
        "cache": cache.stats(),
    });
    Ok((items, summary))
}
