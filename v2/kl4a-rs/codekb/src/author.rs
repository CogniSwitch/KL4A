//! Port of `kl4a/codekb/author.py`.
//!
//! The hybrid-mining "code author" step: builds the LLM request for one
//! symbol from bundle state only (never a raw repo file read at request
//! time), and validates whatever candidate claim comes back through three
//! grounding layers before it is ever written as knowledge:
//!
//! 1. shape (required string/list fields present and well-formed)
//! 2. closed id vocabulary (every cited symbol/relation id must be one the
//!    request actually offered — no invented ids)
//! 3. quoted-code anchor (the claim's `quoted_code` must be a verbatim
//!    substring of the symbol's own source)
//!
//! A 4th layer (existing bundle validation) runs later over whatever
//! survives here, outside this file's scope.
//!
//! Ported symbol-for-symbol from the Python source (verified via tools-code
//! MCP `code_symbols_get`). Every one of `validate_candidate`'s sequential
//! checks is its own line below, in the same order as the Python, not
//! collapsed into "the common cases."
//!
//! ## Cross-batch dependencies (NOT part of this batch — report only)
//!
//! - `kl4a.codekb.state.{STATE_DIR, read_json}` (source file
//!   `kl4a/codekb/state.py`, not in this batch) is expected here as
//!   `crate::state::{STATE_DIR, read_json}` — `read_symbol_source` below
//!   reads `<bundle_dir>/<STATE_DIR>/code_inventory.json` through it,
//!   exactly as the Python does.
//! - `kl4a.kl4a.llm_provider.{llm_settings, parse_author_response}` (source
//!   file `kl4a/kl4a/llm_provider.py`, a *different, shared* package not
//!   part of any codekb batch) backs `azure_code_author` only. Python calls
//!   `llm_settings.complete(messages)` on a module-level settings singleton
//!   then `parse_author_response(text)`. Modeled here as two free functions,
//!   `crate::llm_provider::llm_settings_complete` and
//!   `crate::llm_provider::parse_author_response`, since this batch has no
//!   MCP evidence for `llm_provider.py`'s actual Rust-side shape (no
//!   singleton/config wiring is invented here) —
//!   `// UNCONFIRMED: exact llm_provider signature, needs verification
//!   against kl4a/kl4a/llm_provider.py once that batch lands`.
//!
//! Neither module exists yet in `v2/kl4a-rs`; this file compiles once whichever
//! batch ports them adds the matching `pub mod` to `codekb`'s `lib.rs`. See
//! the handback report for the exact ledger entries.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use once_cell::sync::Lazy;
use serde_json::{json, Value};

use crate::state::{read_json, STATE_DIR};

/// Port of `kl4a.codekb.author.CLAIM_KINDS`.
pub static CLAIM_KINDS: Lazy<BTreeSet<&'static str>> = Lazy::new(|| {
    [
        "purpose",
        "behavior",
        "side-effect",
        "failure-mode",
        "workflow-role",
        "change-risk",
        "domain-concept",
    ]
    .into_iter()
    .collect()
});

/// Port of `kl4a.codekb.author.CODE_AUTHOR_SYSTEM_PROMPT`, verbatim.
pub const CODE_AUTHOR_SYSTEM_PROMPT: &str = "You are an OKF v0.2 author for source-code knowledge.
Return only a JSON object, with no Markdown fences and no commentary.

The JSON object must have:
- knowledge_items: list of claim objects

Every knowledge_items entry must include:
  claim_kind: one of purpose, behavior, side-effect, failure-mode, workflow-role, change-risk, domain-concept
  title: short noun phrase naming the claim
  claim: one or two sentences of prose
  symbols: list of symbol ids, each copied from allowed_symbol_ids
  relations: list of relation ids, each copied from allowed_relation_ids
  quoted_code: an exact substring copied character-for-character from symbol.source
  confidence: number between 0 and 1

Hard rules:
- You may only cite ids that appear in allowed_symbol_ids and allowed_relation_ids.
  Never invent, guess, complete, or normalize an id. If no allowed id supports a
  claim, omit that claim.
- quoted_code must be copied verbatim from the provided source. Do not reformat,
  re-indent, elide with ellipses, or paraphrase it.
- Describe only what the provided source and relations show. Do not describe code
  you were not given, and do not assume a symbol exists because its name suggests it.
- At most one claim per claim_kind per symbol.
- If the symbol carries no interpretive content beyond its docstring, return an
  empty knowledge_items list. An empty list is a valid and useful answer.
- Never state that a claim is approved, verified, or safe to rely on.
";

/// Port of `kl4a.codekb.author.ClaimRejected`.
///
/// A distinct error type (not a generic `anyhow::Error`) because callers
/// branch on it specifically: a rejected candidate is recorded with its
/// reason and mining continues, rather than the whole mining run aborting —
/// exactly the "genuinely distinct exception type the code branches on"
/// case for `thiserror`.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ClaimRejected(pub String);

impl ClaimRejected {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// Port of `kl4a.codekb.author.read_symbol_source`.
///
/// Returns the symbol's exact source lines, which are also the grounding
/// corpus. Returns `""` (never an error) when the source metadata or the
/// referenced file is missing — mirroring Python's silent `return ""` on
/// both failure paths.
pub fn read_symbol_source(bundle_dir: &Path, symbol: &Value) -> String {
    let inventory = read_json(
        &bundle_dir.join(STATE_DIR).join("code_inventory.json"),
        json!({"sources": []}),
    );
    let source_id = symbol.get("source_id").and_then(Value::as_str);
    let source = inventory
        .get("sources")
        .and_then(Value::as_array)
        .and_then(|sources| {
            sources.iter().find(|item| {
                item.get("id").and_then(Value::as_str) == source_id
            })
        });
    let Some(source) = source else {
        return String::new();
    };
    let Some(original_path) = source.get("original_path").and_then(Value::as_str) else {
        return String::new();
    };
    let path = bundle_dir.join(original_path);
    if !path.exists() {
        return String::new();
    }
    let Ok(text) = std::fs::read_to_string(&path) else {
        return String::new();
    };
    let lines: Vec<&str> = text.lines().collect();
    let line_start = symbol
        .get("line_start")
        .and_then(Value::as_i64)
        .unwrap_or(1);
    let start = (line_start - 1).max(0) as usize;
    let default_end = start as i64 + 1;
    let line_end = symbol
        .get("line_end")
        .and_then(Value::as_i64)
        .unwrap_or(default_end);
    let end = (line_end.max(0) as usize).min(lines.len());
    if start >= end {
        return String::new();
    }
    lines[start..end].join("\n")
}

/// Port of `kl4a.codekb.author.build_symbol_author_request`.
///
/// Builds the LLM request from bundle state only. The model never sees a
/// raw repository file; it sees the symbol's own source range and the
/// static relations already resolved for it, which is exactly the material
/// the grounding checks in [`validate_candidate`] can later adjudicate a
/// response against.
pub fn build_symbol_author_request(
    bundle_dir: &Path,
    symbol: &Value,
    relations: &[Value],
    neighborhood: &[Value],
) -> Value {
    let source = read_symbol_source(bundle_dir, symbol);

    let mut allowed_symbol_ids: BTreeSet<String> = neighborhood
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    if let Some(id) = symbol.get("id").and_then(Value::as_str) {
        allowed_symbol_ids.insert(id.to_string());
    }

    let allowed_relation_ids: BTreeSet<String> = relations
        .iter()
        .filter_map(|relation| relation.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .collect();

    let relations_view: Vec<Value> = relations
        .iter()
        .map(|relation| {
            json!({
                "id": relation.get("id"),
                "predicate": relation.get("predicate"),
                "object": relation.get("object"),
                "object_kind": relation.get("object_kind"),
                "resolution_status": relation
                    .get("relation")
                    .and_then(|r| r.get("resolution_status")),
            })
        })
        .collect();

    let neighborhood_view: Vec<Value> = neighborhood
        .iter()
        .map(|item| {
            json!({
                "id": item.get("id"),
                "qualified_name": item.get("qualified_name"),
                "kind": item.get("kind"),
            })
        })
        .collect();

    json!({
        "instruction": "Author OKF code knowledge claims for this symbol. Cite only the supplied \
ids and quote only the supplied source. Return JSON with knowledge_items.",
        "symbol": {
            "id": symbol.get("id"),
            "qualified_name": symbol.get("qualified_name"),
            "kind": symbol.get("kind"),
            "language": symbol.get("language").and_then(Value::as_str).unwrap_or("python"),
            "file": symbol.get("file"),
            "signature": symbol.get("signature"),
            "docstring": symbol.get("docstring").and_then(Value::as_str).unwrap_or(""),
            "line_start": symbol.get("line_start"),
            "line_end": symbol.get("line_end"),
            "source": source,
        },
        "relations": relations_view,
        "neighborhood": neighborhood_view,
        "allowed_symbol_ids": allowed_symbol_ids.into_iter().collect::<Vec<_>>(),
        "allowed_relation_ids": allowed_relation_ids.into_iter().collect::<Vec<_>>(),
    })
}

/// Port of `kl4a.codekb.author.require_string`.
pub fn require_string(data: &Value, key: &str) -> Result<String, ClaimRejected> {
    match data.get(key).and_then(Value::as_str) {
        Some(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        _ => Err(ClaimRejected::new(format!(
            "missing required string field: {key}"
        ))),
    }
}

/// Port of `kl4a.codekb.author.require_id_list`.
pub fn require_id_list(
    data: &Value,
    key: &str,
    allow_empty: bool,
) -> Result<Vec<String>, ClaimRejected> {
    let raw = data.get(key).cloned().unwrap_or(Value::Array(vec![]));
    let items = match &raw {
        Value::Null => Vec::new(),
        Value::Array(items) => items.clone(),
        _ => {
            return Err(ClaimRejected::new(format!(
                "{key} must be a list of non-empty strings"
            )))
        }
    };
    let mut strings = Vec::with_capacity(items.len());
    for item in &items {
        match item.as_str() {
            Some(s) if !s.trim().is_empty() => strings.push(s.trim().to_string()),
            _ => {
                return Err(ClaimRejected::new(format!(
                    "{key} must be a list of non-empty strings"
                )))
            }
        }
    }
    if strings.is_empty() && !allow_empty {
        return Err(ClaimRejected::new(format!("{key} must not be empty")));
    }
    // Python: sorted(dict.fromkeys(...)) — de-dup preserving nothing but
    // order-of-first-seen is discarded anyway by the subsequent `sorted`.
    let deduped: BTreeSet<String> = strings.into_iter().collect();
    Ok(deduped.into_iter().collect())
}

/// Port of `kl4a.codekb.author.clamp_confidence`.
pub fn clamp_confidence(value: Option<&Value>) -> f64 {
    let parsed = value.and_then(|v| {
        if let Some(f) = v.as_f64() {
            Some(f)
        } else if let Some(s) = v.as_str() {
            s.parse::<f64>().ok()
        } else {
            None
        }
    });
    match parsed {
        Some(confidence) => confidence.clamp(0.0, 1.0),
        None => 0.6,
    }
}

/// Port of `kl4a.codekb.author.validate_candidate`.
///
/// Applies the grounding layers to one candidate claim, in the same order
/// as the Python: layer 1 (shape) via [`require_string`]/[`require_id_list`],
/// layer 2 (closed id vocabulary), layer 3 (quoted-code anchor). Layer 4
/// (existing bundle validation) is out of scope for this function, same as
/// the Python.
///
/// `require_quoted_code` defaults to `true` in Python
/// (`require_quoted_code: bool = True`); callers pass it explicitly here.
pub fn validate_candidate(
    candidate: &Value,
    request: &Value,
    require_quoted_code: bool,
) -> Result<Value, ClaimRejected> {
    let Value::Object(_) = candidate else {
        return Err(ClaimRejected::new("candidate is not an object"));
    };

    let claim_kind = require_string(candidate, "claim_kind")?;
    if !CLAIM_KINDS.contains(claim_kind.as_str()) {
        return Err(ClaimRejected::new(format!(
            "unknown claim_kind: {claim_kind}"
        )));
    }
    let title = require_string(candidate, "title")?;
    let claim = require_string(candidate, "claim")?;

    let allowed_symbols: BTreeSet<String> = request
        .get("allowed_symbol_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    let allowed_relations: BTreeSet<String> = request
        .get("allowed_relation_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();

    let symbols = require_id_list(candidate, "symbols", false)?;
    let relations = require_id_list(candidate, "relations", true)?;

    let invented_symbols: Vec<&String> = symbols
        .iter()
        .filter(|item| !allowed_symbols.contains(*item))
        .collect();
    if !invented_symbols.is_empty() {
        let joined = invented_symbols
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(ClaimRejected::new(format!(
            "cites symbol id(s) outside allowed vocabulary: {joined}"
        )));
    }
    let invented_relations: Vec<&String> = relations
        .iter()
        .filter(|item| !allowed_relations.contains(*item))
        .collect();
    if !invented_relations.is_empty() {
        let joined = invented_relations
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(ClaimRejected::new(format!(
            "cites relation id(s) outside allowed vocabulary: {joined}"
        )));
    }

    let subject_symbol_id = request
        .get("symbol")
        .and_then(|s| s.get("id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !symbols.iter().any(|s| s == subject_symbol_id) {
        return Err(ClaimRejected::new(
            "claim does not cite the symbol it was authored for",
        ));
    }

    let quoted_code = candidate
        .get("quoted_code")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if require_quoted_code && quoted_code.trim().is_empty() {
        return Err(ClaimRejected::new("missing quoted_code"));
    }
    let symbol_source = request
        .get("symbol")
        .and_then(|s| s.get("source"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let anchored = !quoted_code.trim().is_empty() && symbol_source.contains(&quoted_code);

    Ok(json!({
        "claim_kind": claim_kind,
        "title": title,
        "claim": claim,
        "symbols": symbols,
        "relations": relations,
        "quoted_code": quoted_code,
        "anchored": anchored,
        "confidence": clamp_confidence(candidate.get("confidence")),
    }))
}

/// Port of `kl4a.codekb.author.code_knowledge_id_for_claim`.
///
/// Derives a claim id from the symbol and claim kind, never from generated
/// text: review decisions are keyed on this id, so it has to stay stable
/// across re-mines even though the prose behind it will not be
/// byte-identical.
pub fn code_knowledge_id_for_claim(symbol_id: &str, claim_kind: &str, ordinal: i64) -> String {
    let suffix = if ordinal == 0 {
        format!("llm-{claim_kind}")
    } else {
        format!("llm-{claim_kind}-{}", ordinal + 1)
    };
    crate::ids::bounded_id("ki", &format!("{symbol_id}-{suffix}"), 120)
}

/// Port of `kl4a.codekb.author.build_code_author_messages`.
pub fn build_code_author_messages(request: &Value) -> Vec<Value> {
    let pretty = serde_json::to_string_pretty(request).unwrap_or_default();
    vec![
        json!({"role": "system", "content": CODE_AUTHOR_SYSTEM_PROMPT}),
        json!({
            "role": "user",
            "content": format!(
                "Author OKF code knowledge for this symbol. Return only JSON matching \
the required shape.\n\n{pretty}"
            ),
        }),
    ]
}

/// Port of `kl4a.codekb.author.azure_code_author`.
///
/// `// UNCONFIRMED:` the exact `llm_settings`/`parse_author_response` call
/// shape — see the module-level cross-batch note. This mirrors the Python
/// two-step pipeline (`build_code_author_messages` then `complete` then
/// `parse_author_response`) exactly; only the two external functions'
/// concrete signatures are unverified pending that batch.
pub fn azure_code_author(request: &Value) -> Result<Value> {
    let messages = build_code_author_messages(request);
    let text = crate::llm_settings::complete(&messages, None)?;
    crate::llm_provider::parse_author_response(&text)
}

/// Port of the `CodeAuthorFn` type alias used across `kl4a.codekb.knowledge`
/// / `kl4a.codekb.procedures` (`author: CodeAuthorFn`) — a pluggable
/// request-to-response author callback, of which [`azure_code_author`] is
/// one implementation (the fixture/test author functions seen in the test
/// suite are others, out of scope for this batch).
pub type CodeAuthorFn = dyn Fn(&Value) -> Result<Value> + Send + Sync;
