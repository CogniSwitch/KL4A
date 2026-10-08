//! Port of `tools/sopkb/sopkb/okf_author.py`. docs/port/port-mapping-b-mining-settings.md
//! §3.2. HTTP mechanics (URL building, headers, timeout, JSON body encoding, response
//! text extraction) already live in `sopkb-llm::author_call`/`author_call_with_transport`
//! -- this module only builds the request payload, builds the `[{role,content}]`
//! message list, and parses/applies the JSON the model returns.
//!
//! `frontmatter` values flow through `crate::ordered_json::OrderedJson` end-to-end
//! (parse -> apply -> `okf_writer`) to preserve the LLM's original JSON key order into
//! the YAML file on disk -- see `crate::ordered_json`'s module doc for why.
//!
//! **P-M31 (transport error mapping/redaction) is likewise not touched here.**
//! `sopkb_llm::post_and_parse` already wraps a non-2xx HTTP status as
//! `"Azure OpenAI {label} request failed: HTTP {status}: {body}"` with the error body
//! included in full (no truncation, no secret redaction), and a connection-level
//! failure (`TransportIoError`) propagates its native message unwrapped -- both exactly
//! mirroring Python's "only `HTTPError` is caught" behavior. PORT_PLAN.md marks this
//! **FIX** ("map transport errors to `Upstream`; truncate and redact the body"), but
//! that error-shape decision belongs to `sopkb_llm` (the crate that actually owns the
//! HTTP call and constructs the message), not this crate, which only ever sees
//! `sopkb_llm`'s already-formatted `Result`. Reaching into `sopkb_llm` to change its
//! error formatting is out of this worktree's scope per this crate's task brief; noted
//! here rather than silently ignored.

use crate::okf_writer::{self, PreparedDoc};
use crate::ordered_json::{parse_ordered_json, OrderedJson};
use serde_json::{json, Value};
use sopkb_core::error::{Result, SopkbError};
use sopkb_fmt::{CharIndex, OrderedMap};
use sopkb_llm::Message;
use std::path::Path;

/// `(items, writes, staged_docs, next_ordinal)` -- see `apply_author_response` below.
type AuthorApplyResult = (Vec<Value>, Vec<Value>, Vec<PreparedDoc>, u32);

/// Byte-exact port of `okf_author.AUTHOR_SYSTEM_PROMPT` (P-M17). Generated directly
/// from `tools/sopkb/sopkb/okf_author.py`'s triple-quoted string (not hand-transcribed)
/// -- see `tests/author_prompt_matches_python.rs`, which independently re-derives this
/// same text from that Python source file at test time and asserts byte equality, so
/// any future drift between the two is caught. Written with explicit `\n` escapes
/// (never a raw embedded newline) so the constant's value can never be altered by
/// git's line-ending conversion of this .rs file on checkout.
pub const AUTHOR_SYSTEM_PROMPT: &str = "You are an OKF v0.2 author for SOP knowledge.\nReturn only a JSON object, with no Markdown fences and no commentary.\nThe JSON object must have:\n- documents: list of OKF document objects {concept_id, frontmatter, body}\n- knowledge_items: list of knowledge item objects\n\nRules:\n- Use exact source_text copied from the provided section text.\n- Every knowledge_items entry must include subject, predicate, object, source_text, and decision_rules.\n- If a section contains no SOP obligation or decision rule, return empty lists.\n- Split compound SOP statements into separate decision rules when needed.\n- Use frontmatter.type for every document.\n- Put provenance in sources frontmatter, not in a citations section.\n- Every sources entry must include id, title, and resource. Use the supplied source.resource value when no better resource is available.\n- Only documents with type \"SOP Decision Rule\" may include frontmatter.sopkb.rule.\n- Decision rule documents must include frontmatter.sopkb.rule with:\n  id: string\n  obligation: {fact: string, action: string, label: string}\n  condition: optional {fact: string, operator: \"is_true\" or \"is_false\", label: string}\n  otherwise: optional {action_required: boolean, label: string}\n- Proposed output must use review_status: proposed.\n\nMinimal valid knowledge item:\n{\"subject\":\"Procedure\",\"predicate\":\"should\",\"object\":\"Staff should record contraindications.\",\"source_text\":\"Staff should record contraindications.\",\"review_status\":\"proposed\",\"decision_rules\":[{\"id\":\"rule-record-contraindications\",\"title\":\"Record contraindications\",\"obligation\":{\"fact\":\"contraindications_recorded\",\"action\":\"record\",\"label\":\"Contraindications recorded\"}}]}\n\nMinimal valid decision rule document:\n{\"concept_id\":\"rules/rule-record-contraindications\",\"frontmatter\":{\"type\":\"SOP Decision Rule\",\"title\":\"Record contraindications\",\"sources\":[{\"id\":\"source\",\"title\":\"Source\",\"resource\":\"source\"}],\"sopkb\":{\"rule\":{\"id\":\"rule-record-contraindications\",\"obligation\":{\"fact\":\"contraindications_recorded\",\"action\":\"record\",\"label\":\"Contraindications recorded\"}}}},\"body\":\"# Record contraindications\n\"}\n";

/// `content[start:end]` in Python `str` (char) units, same clamping as `mine_fixture`.
fn char_slice(content: &str, start: usize, end: usize) -> String {
    let idx = CharIndex::new(content);
    let len = idx.char_len();
    let start = start.min(len);
    let end = end.min(len).max(start);
    let start_b = idx.byte_offset_at_char(start);
    let end_b = idx.byte_offset_at_char(end);
    content[start_b..end_b].to_string()
}

/// `build_section_author_request`. `section` is the raw `sections.json` entry (a
/// `serde_json::Value` -- order doesn't matter for anything read out of it).
///
/// P-M15 (lean FIX): Python reads `source.get("path")`, a key inventory records never
/// have (they carry `original_path`/`normalized_path`), so `resource` always falls
/// back to `source_id` in the original. This port reads `original_path` instead, which
/// actually exists -- see DEVIATIONS.md for the resulting prompt-payload change.
pub fn build_section_author_request(bundle_dir: &Path, section: &Value) -> Result<Value> {
    let normalized_path = section
        .get("normalized_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SopkbError::Value("section missing normalized_path".to_string()))?;
    let content = sopkb_core::store::read_text_universal_newlines(&bundle_dir.join(normalized_path))?;
    let start_pos = section.get("start_pos").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let end_pos = section.get("end_pos").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let section_text = char_slice(&content, start_pos, end_pos);

    let inventory = sopkb_core::store::read_state_json(bundle_dir, "inventory.json", json!({"sources": []}))?;
    let source_id = section.get("source_id").and_then(|v| v.as_str()).unwrap_or_default();
    let empty_source = json!({});
    let source = inventory
        .get("sources")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.iter().find(|s| s.get("id").and_then(|i| i.as_str()) == Some(source_id)))
        .unwrap_or(&empty_source);

    let title = source.get("title").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).unwrap_or(source_id);
    // P-M15: `original_path`, not the non-existent `path` key -- see DEVIATIONS.md.
    let resource = source.get("original_path").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).unwrap_or(source_id);

    Ok(json!({
        "instruction": "Author OKF-compatible SOP knowledge from this source section. Return JSON with documents and knowledge_items. Documents must contain concept_id, frontmatter, and body. Decision rules must use frontmatter sopkb.rule with id, obligation, optional condition, and optional otherwise.",
        "section": {
            "id": section.get("id"),
            "source_id": source_id,
            "heading": section.get("heading"),
            "semantic_role": section.get("semantic_role"),
            "text": section_text,
        },
        "source": {
            "id": source_id,
            "title": title,
            "resource": resource,
        },
    }))
}

/// `build_author_messages`. `mining_prompt` has no env override and no `DEFAULTS`
/// entry, so `resolve` returns the stored profile value (stripped) or `""`; a non-blank
/// value fully REPLACES `AUTHOR_SYSTEM_PROMPT` (P-M18 -- no prepend/append/templating).
///
/// `bundle_override`, when non-blank, wins over even a non-blank profile override --
/// see `sopkb_core::prompt_overrides`'s own doc comment for why a per-BUNDLE override
/// exists at all (a global per-profile override affects every bundle using that
/// profile; this affects only the one bundle it's set on).
pub fn build_author_messages(request: &Value, profile_id: Option<&str>, bundle_override: Option<&str>) -> Vec<Message> {
    let system_prompt = match bundle_override.map(str::trim).filter(|s| !s.is_empty()) {
        Some(over) => over.to_string(),
        None => {
            let resolved = sopkb_config::resolve("mining_prompt", profile_id);
            if resolved.is_empty() { AUTHOR_SYSTEM_PROMPT.to_string() } else { resolved }
        }
    };
    let user_content = format!(
        "Author OKF SOP knowledge for this normalized source section. Return only JSON matching the required shape.\n\n{}",
        sopkb_fmt::to_canonical_json(request)
    );
    vec![Message::system(system_prompt), Message::user(user_content)]
}

/// Strips a single surrounding Markdown code fence (```` ```json ... ``` ```` or
/// ```` ``` ... ``` ````) if present, before handing text to the JSON parser (P-M12
/// FIX -- see DEVIATIONS.md). Only a fence that wraps the ENTIRE response is stripped;
/// this is not a "find JSON anywhere in the text" heuristic.
fn strip_markdown_fence(text: &str) -> &str {
    let trimmed = text.trim();
    let Some(after_open) = trimmed.strip_prefix("```") else { return text };
    // Skip an optional language tag (e.g. "json") up to the first newline.
    let after_lang = match after_open.find('\n') {
        Some(idx) => &after_open[idx + 1..],
        None => return text, // no body line at all -- not a real fence, leave as-is
    };
    let Some(body) = after_lang.strip_suffix("```") else { return text };
    body.trim_end_matches(['\n', '\r', ' ', '\t'])
}

/// `parse_author_response`: strict JSON parse (after fence stripping) that must yield a
/// top-level object.
pub fn parse_author_response(text: &str) -> Result<OrderedMap<OrderedJson>> {
    let stripped = strip_markdown_fence(text);
    let value = parse_ordered_json(stripped).map_err(|e| SopkbError::Value(format!("LLM author response must be JSON: {e}")))?;
    match value {
        OrderedJson::Object(map) => Ok(map),
        _ => Err(SopkbError::Value("LLM author response must be a JSON object".to_string())),
    }
}

fn require_string(data: &OrderedMap<OrderedJson>, key: &str) -> Result<String> {
    match data.get(key) {
        Some(OrderedJson::String(s)) if !s.trim().is_empty() => Ok(s.clone()),
        _ => Err(SopkbError::Value(format!("missing required string field: {key}"))),
    }
}

/// `azure_llm_author` = `parse_author_response(call_azure_responses(build_author_messages(request)))`,
/// with `call_azure_responses`'s HTTP mechanics delegated entirely to `sopkb_llm`.
///
/// **P-M13, closed 2026-08-22.** Python's `okf_author.response_text` had no
/// `status == "incomplete"` guard (unlike `agent_chat.response_text`), so a
/// `max_output_tokens`-truncated response fell through to `output_text`/`output`
/// extraction and surfaced later as a confusing JSON parse error (or, as observed in a
/// real mining run, a "missing required frontmatter key" validation error) rather than
/// an actionable one. PORT_PLAN.md marked this **FIX** ("add the guard, with
/// `agent_chat`'s message"). The guard now lives in `sopkb_llm::response`'s
/// `response_text_author` -- the correct layer, since that's where `status`/
/// `incomplete_details`/`usage` are actually available; this crate never needed to see
/// the raw response body, so no boundary was crossed and no public API changed. This
/// module's `azure_llm_author`/`azure_llm_author_with_transport` get the fix for free.
pub fn azure_llm_author(request: &Value, profile_id: Option<&str>, bundle_override: Option<&str>) -> Result<OrderedMap<OrderedJson>> {
    azure_llm_author_with_transport(request, profile_id, bundle_override, &sopkb_llm::UreqTransport)
}

/// Same as `azure_llm_author` but with an injectable transport, for tests and the
/// recorded-response harness (no live LLM call belongs in `cargo test`).
pub fn azure_llm_author_with_transport(
    request: &Value,
    profile_id: Option<&str>,
    bundle_override: Option<&str>,
    transport: &dyn sopkb_llm::Transport,
) -> Result<OrderedMap<OrderedJson>> {
    let messages = build_author_messages(request, profile_id, bundle_override);
    let text = sopkb_llm::author_call_with_transport(&messages, profile_id, transport)?;
    parse_author_response(&text)
}

/// `coerce_list`: `response.get(field) or []` falsy-coercion (P-M19 PRESERVE) --
/// `null`/`false`/`0`/`""`/`{}`/`[]` all silently become `[]`; only a truthy non-list
/// raises.
fn coerce_list(value: Option<&OrderedJson>, field_name: &str) -> Result<Vec<OrderedJson>> {
    match value {
        None => Ok(vec![]),
        Some(v) if !v.is_truthy() => Ok(vec![]),
        Some(OrderedJson::Array(items)) => Ok(items.clone()),
        Some(_) => Err(SopkbError::Value(format!("LLM author response {field_name} must be a list"))),
    }
}

/// `write_authored_document`, minus the disk write (staged via `okf_writer::prepare_concept_doc`
/// -- see `mine_with_author`'s doc comment for why writes are staged, P-M11 FIX).
///
/// P-M19 FIX: Python's `document.get(...)` on a non-mapping `document` element raises a
/// raw `AttributeError`; this returns a typed error instead.
fn prepare_authored_document(document: &OrderedJson, actor: &str) -> Result<PreparedDoc> {
    let doc_obj = document.as_object().ok_or_else(|| SopkbError::Value("authored document must be a mapping".to_string()))?;
    let concept_id = require_string(doc_obj, "concept_id")?;
    let frontmatter = doc_obj
        .get("frontmatter")
        .and_then(|v| v.as_object())
        .ok_or_else(|| SopkbError::Value("authored document frontmatter must be a mapping".to_string()))?;
    let body = doc_obj.get("body").and_then(|v| v.as_str()).ok_or_else(|| SopkbError::Value("authored document body must be a string".to_string()))?;
    okf_writer::prepare_concept_doc(&concept_id, frontmatter, body, actor)
}

/// `authored_knowledge_item`, returning the already-`serde_json`-serialized dict shape
/// (field order doesn't matter -- `write_json` sorts alphabetically regardless).
///
/// P-M19 FIX: Python's `candidate.get(...)` on a non-mapping `candidate` raises a raw
/// `AttributeError`; typed error here instead.
fn authored_knowledge_item(section: &Value, section_text: &str, section_start_pos: usize, candidate: &OrderedJson, ordinal: u32, actor: &str) -> Result<Value> {
    let candidate_obj = candidate.as_object().ok_or_else(|| SopkbError::Value("authored knowledge item must be a mapping".to_string()))?;

    let source_text = require_string(candidate_obj, "source_text")?;
    let (start_pos, end_pos, span_status) = match section_text.find(source_text.as_str()) {
        Some(byte_idx) => {
            let char_idx = CharIndex::new(section_text);
            let relative_start_char = char_idx.char_offset_at_byte(byte_idx);
            let start = section_start_pos + relative_start_char;
            let end = start + source_text.chars().count();
            (Some(start), Some(end), "exact")
        }
        None => (None, None, "llm_claimed"),
    };

    let mut metadata: serde_json::Map<String, Value> = match candidate_obj.get("metadata") {
        Some(v) if v.is_truthy() => match v.as_object() {
            Some(obj) => OrderedJson::Object(obj.clone()).to_json_map(),
            // Python's `dict(candidate.get("metadata") or {})` on a truthy non-mapping
            // (e.g. a string) raises its own raw crash (`dictionary update sequence
            // element` / similar); converted to a typed error here in the same spirit
            // as the other P-M19/P-M26 raw-crash fixes.
            None => return Err(SopkbError::Value("authored knowledge item metadata must be a mapping".to_string())),
        },
        _ => serde_json::Map::new(),
    };
    metadata.insert("provider".to_string(), json!(actor)); // OVERWRITES any model-supplied "provider" (P-M20 PRESERVE).
    if let Some(OrderedJson::Array(rules)) = candidate_obj.get("decision_rules") {
        let rules_json: Vec<Value> = rules.iter().map(|r| r.to_json_value()).collect();
        metadata.insert("decision_rules".to_string(), Value::Array(rules_json)); // copied verbatim, zero schema validation.
    }

    let item_type = match candidate_obj.get("item_type") {
        Some(v) if v.is_truthy() => v.py_str(),
        _ => "claim".to_string(),
    };
    let subject = require_string(candidate_obj, "subject")?;
    let predicate = require_string(candidate_obj, "predicate")?;
    let object = require_string(candidate_obj, "object")?;
    let confidence = match candidate_obj.get("confidence") {
        // `candidate.get("confidence") or 0.7`: an explicit 0/0.0/false/"" confidence
        // silently becomes 0.7 (P-M19-adjacent PRESERVE, documented in the pseudocode).
        Some(v) if v.is_truthy() => v.py_float_of().map_err(SopkbError::Value)?,
        _ => 0.7,
    };
    let review_status = match candidate_obj.get("review_status") {
        Some(v) if v.is_truthy() => v.py_str(),
        _ => "proposed".to_string(),
    };

    let source_id = section.get("source_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let section_id = section.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();

    let item = sopkb_core::models::KnowledgeItem {
        // Version-keyed, same as the fixture miner -- see `mine_fixture.rs`.
        id: sopkb_core::ids::knowledge_item_id_for(
            &sopkb_core::knowledge_lifecycle::item_source_key_for(section),
            ordinal,
        ),
        item_type,
        subject,
        predicate,
        object,
        source_id,
        section_id,
        source_text,
        start_pos,
        end_pos,
        span_status: span_status.to_string(),
        derivation: "llm_authored".to_string(),
        confidence,
        review_status,
        metadata,
        source_version_id: section.get("source_version_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
        lifecycle_status: "active".to_string(),
        supersedes: Vec::new(),
        superseded_by: Vec::new(),
    };
    Ok(serde_json::to_value(item).unwrap())
}

/// `apply_author_response`, split into "compute" (this function) with disk writes
/// deferred to the caller via the returned `Vec<PreparedDoc>` (P-M11 FIX staging).
///
/// Returns `(items, writes, staged_docs, next_ordinal)`. Documents are processed before
/// knowledge items (P-M21 PRESERVE -- order matters for reasoning about partial state,
/// even though staging now makes any one section's failure fully non-destructive).
fn apply_author_response(
    bundle_dir: &Path,
    section: &Value,
    response: &OrderedMap<OrderedJson>,
    ordinal_start: u32,
    actor: &str,
) -> Result<AuthorApplyResult> {
    let documents = coerce_list(response.get("documents"), "documents")?;
    let knowledge_items = coerce_list(response.get("knowledge_items"), "knowledge_items")?;

    let mut writes = Vec::new();
    let mut staged = Vec::new();
    for document in &documents {
        let prepared = prepare_authored_document(document, actor)?;
        writes.push(prepared.to_write_entry());
        staged.push(prepared);
    }

    let normalized_path = section
        .get("normalized_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SopkbError::Value("section missing normalized_path".to_string()))?;
    let content = sopkb_core::store::read_text_universal_newlines(&bundle_dir.join(normalized_path))?;
    let start_pos = section.get("start_pos").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let end_pos = section.get("end_pos").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let section_text = char_slice(&content, start_pos, end_pos);

    let mut items = Vec::new();
    let mut ordinal = ordinal_start;
    for candidate in &knowledge_items {
        let item = authored_knowledge_item(section, &section_text, start_pos, candidate, ordinal, actor)?;
        items.push(item);
        ordinal += 1;
    }
    Ok((items, writes, staged, ordinal))
}

/// Pure, side-effect-free structural check of a raw author() response -- mirrors real
/// oss-launch's `_validate_author_response` (`okf_author.py`, added after this port's
/// original P-M11 fix was written; see `mine_with_author`'s doc comment for the full
/// story). Returns a cleaned copy with any malformed `documents`/`knowledge_items`
/// entry dropped, plus one message per dropped entry.
///
/// A field that is present but a truthy non-list (e.g. `"documents": "oops"`) is a
/// deeper shape problem than a single bad entry -- matching Python, the response is
/// returned UNMODIFIED in that case (the bad value is not replaced with `[]`), so the
/// caller's later `apply_author_response`/`coerce_list` call still raises its own typed
/// error for it. A missing or falsy field (`null`/`false`/`0`/`""`/`{}`/`[]`) silently
/// coerces to `[]`, same as `coerce_list`.
fn validate_author_response(response: &OrderedMap<OrderedJson>) -> (OrderedMap<OrderedJson>, Vec<String>) {
    for field in ["documents", "knowledge_items"] {
        if let Some(value) = response.get(field) {
            if value.is_truthy() && !matches!(value, OrderedJson::Array(_)) {
                return (response.clone(), vec![format!("{field} must be a list")]);
            }
        }
    }

    let documents: Vec<OrderedJson> = match response.get("documents") {
        Some(OrderedJson::Array(items)) => items.clone(),
        _ => vec![],
    };
    let knowledge_items: Vec<OrderedJson> = match response.get("knowledge_items") {
        Some(OrderedJson::Array(items)) => items.clone(),
        _ => vec![],
    };

    let mut errors = Vec::new();
    let mut valid_documents = Vec::new();
    for document in documents {
        match validate_document_shape(&document) {
            Ok(()) => valid_documents.push(document),
            Err(msg) => errors.push(format!("document dropped: {msg}")),
        }
    }

    let mut valid_items = Vec::new();
    for candidate in knowledge_items {
        match validate_knowledge_item_shape(&candidate) {
            Ok(()) => valid_items.push(candidate),
            Err(msg) => errors.push(format!("knowledge item dropped: {msg}")),
        }
    }

    let mut cleaned = response.clone();
    cleaned.insert("documents", OrderedJson::Array(valid_documents));
    cleaned.insert("knowledge_items", OrderedJson::Array(valid_items));
    (cleaned, errors)
}

fn validate_document_shape(document: &OrderedJson) -> std::result::Result<(), String> {
    let obj = document.as_object().ok_or_else(|| "must be a mapping".to_string())?;
    require_string(obj, "concept_id").map_err(|e| e.to_string())?;
    match obj.get("frontmatter") {
        Some(OrderedJson::Object(_)) => {}
        _ => return Err("frontmatter must be a mapping".to_string()),
    }
    match obj.get("body") {
        Some(OrderedJson::String(_)) => {}
        _ => return Err("body must be a string".to_string()),
    }
    Ok(())
}

fn validate_knowledge_item_shape(candidate: &OrderedJson) -> std::result::Result<(), String> {
    let obj = candidate.as_object().ok_or_else(|| "must be a mapping".to_string())?;
    for field in ["subject", "predicate", "object", "source_text"] {
        require_string(obj, field).map_err(|e| e.to_string())?;
    }
    Ok(())
}

const MAX_AUTHOR_ATTEMPTS: u32 = 3;

/// One section's Step 1 outcome: the validated (possibly salvaged) response, or
/// `None` with the reason it couldn't be produced after every retry.
type SectionOutcome = (Option<OrderedMap<OrderedJson>>, Option<String>);

/// `mine_with_author`. `author` is called once per section.
///
/// **Updated 2026-08-24 to match a real oss-launch fix that landed after this port's
/// original P-M11 fix was written** (`docs/port/CATCHUP_PLAN.md`'s tools/sopkb
/// staleness audit). P-M11's all-or-nothing design -- ANY section's failure discarded
/// EVERY section's output, even sections that fully succeeded -- is exactly the
/// failure a real user hit this session ("mining failed for 1 of 1 section(s), nothing
/// written"), and it became much more likely to trigger once
/// `sopkb_workbench::heading_restructure` started splitting documents into many more,
/// smaller sections: more sections means a higher chance at least one produces a
/// response the model can't fill in meaningfully (e.g. a heading picked up from a
/// garbled PDF table with only a few characters of real body text).
///
/// oss-launch's own fix (ported here) trades strict atomicity for graceful
/// degradation:
///   1. [`validate_author_response`] drops individually malformed `documents`/
///      `knowledge_items` entries instead of failing the whole response over one bad
///      candidate.
///   2. Up to [`MAX_AUTHOR_ATTEMPTS`] per section: retry on a transport error, or on a
///      response that still has validation errors, hoping for a cleaner one; after the
///      last attempt, use whatever survived validation rather than give up entirely.
///   3. A section that still fails after retries (transport error on every attempt, or
///      a deeper failure in `apply_author_response` not caught by the shallow
///      pre-validation) is skipped -- logged, not fatal -- while every OTHER section's
///      output is still written. `writes`/`items`/staged docs are only ever extended
///      from a section's own successful result, so a skipped section can't pollute the
///      others' output.
///
/// **Updated 2026-08-24 again to close the previously-disclosed sequential-mining
/// deviation.** A real user hit a mining run that ran for over an hour with no visible
/// progress on a document heading-restructuring had split into 200+ sections --
/// sequential processing at that new scale (introduced by the very heading-
/// restructuring fix that made mining actually work well) made "slow but working" and
/// "genuinely hung" indistinguishable, since the UI had no way to show anything
/// between the mine step's `started` and `done` events. Ported real oss-launch's own
/// fix for this exact problem: `call_author` (Step 1 -- build the request, run the
/// author()-and-retry loop) fans out across [`MAX_PARALLEL_SECTIONS`] worker threads
/// via [`sopkb_core::parallel::parallel_map`] (Rust's equivalent of
/// `ThreadPoolExecutor(max_workers=6).map(...)`, since nothing in `std` provides that
/// directly). Step 2 (applying each response -- minting item ids from a shared
/// `ordinal` counter, staging OKF documents) stays fully sequential, over the
/// already-fetched responses in original section order, for the same reason real
/// oss-launch keeps it sequential: `ordinal` is a single shared counter (parallel
/// increments would mint colliding ids), and `okf_writer::write_prepared_doc`'s writes
/// are not safe for two sections targeting the same `concept_id` to race on.
///
/// `on_progress`, when given, is called `(sections_completed, total)` each time a
/// worker finishes ONE section's author-call-and-retry cycle (success, salvage, or
/// exhausted retries) -- i.e. during Step 1, which is where all the real wall-clock
/// time is spent, not after-the-fact in Step 2 (which has no network waits left and
/// would otherwise make the whole run appear to jump from 0% to 100% instantly at the
/// very end). The seam this crate uses to let a Tauri command surface real progress
/// events without this crate needing to know Tauri exists -- same pattern as
/// `sopkb_workbench::heading_restructure::provider_hook`.
///
/// `is_cancelled`, when given, is checked once per section, right before that
/// section's own author-call-and-retry cycle would start (never mid-cycle -- there is
/// no cheap way to interrupt an in-flight blocking HTTP call). A section skipped this
/// way is NOT the same as one that failed: `chosen`/`response` stays `None` with a
/// distinct "run was cancelled" reason, so Step 2 skips it exactly like any other
/// missing response, and any sections whose author() call had ALREADY started before
/// cancellation was requested still finish normally and their output is kept -- this
/// is cooperative cancellation ("stop starting new work"), not a hard abort.
///
/// Every per-section warning in this function goes through this helper: `eprintln!`
/// for a dev-mode/CLI console (unchanged), AND `sopkb_core::store::append_ingest_log`
/// so the SAME warning also lands in `.sopkb/ingest.log`, reachable from a packaged
/// GUI build's stderr-less release binary too (see `append_ingest_log`'s own doc
/// comment). `message` must never carry raw section/document text, prompts, model
/// responses, or credentials -- every call site here only interpolates section ids,
/// attempt counts, and error `Display` text.
fn warn_section(bundle_dir: &Path, message: String) {
    eprintln!("{message}");
    sopkb_core::store::append_ingest_log(bundle_dir, &message);
}

pub fn mine_with_author<F>(
    bundle_dir: &Path,
    author: F,
    actor: &str,
    on_progress: Option<&(dyn Fn(usize, usize) + Sync)>,
    is_cancelled: Option<&(dyn Fn() -> bool + Sync)>,
) -> Result<Vec<Value>>
where
    F: Fn(&Value) -> Result<OrderedMap<OrderedJson>> + Sync,
{
    sopkb_core::lifecycle::migrate_source_version_state(bundle_dir)?;
    let sections_value = sopkb_core::store::read_state_json(bundle_dir, "sections.json", json!([]))?;
    let sections: Vec<Value> = sections_value.as_array().cloned().unwrap_or_default();
    let existing_items_value = sopkb_core::store::read_state_json(bundle_dir, "items.json", json!([]))?;
    let existing_items: Vec<Value> = existing_items_value.as_array().cloned().unwrap_or_default();
    let okf_root = sopkb_core::store::state_path(bundle_dir, "authored_okf");
    let total = sections.len();
    let completed = std::sync::atomic::AtomicUsize::new(0);

    // Configurable (Settings) rather than hardcoded, no Python equivalent -- see
    // sopkb_config::settings::DEFAULT_MAX_PARALLEL_WORKERS's own doc comment.
    let max_workers = sopkb_config::max_parallel_workers();
    let responses: Vec<SectionOutcome> = sopkb_core::parallel::parallel_map(&sections, max_workers, |_i, section| -> SectionOutcome {
        let section_label = section.get("id").and_then(|v| v.as_str()).unwrap_or("<unknown>").to_string();

        let outcome = (|| -> SectionOutcome {
            if is_cancelled.is_some_and(|f| f()) {
                warn_section(bundle_dir, format!("[sopkb.mine] section {section_label}: skipped, run was cancelled"));
                return (None, Some("run was cancelled".to_string()));
            }
            let request = match build_section_author_request(bundle_dir, section) {
                Ok(r) => r,
                Err(e) => {
                    warn_section(bundle_dir, format!("[sopkb.mine] section {section_label}: skipped, could not build request: {e}"));
                    return (None, Some(e.to_string()));
                }
            };

            let mut chosen: Option<OrderedMap<OrderedJson>> = None;
            let mut last_err: Option<SopkbError> = None;
            for attempt in 1..=MAX_AUTHOR_ATTEMPTS {
                let response = match author(&request) {
                    Ok(r) => r,
                    Err(e) => {
                        last_err = Some(e);
                        continue;
                    }
                };
                let (cleaned, errors) = validate_author_response(&response);
                if errors.is_empty() {
                    chosen = Some(cleaned);
                    break;
                }
                if attempt == MAX_AUTHOR_ATTEMPTS {
                    warn_section(
                        bundle_dir,
                        format!(
                            "[sopkb.mine] section {section_label}: giving up after {MAX_AUTHOR_ATTEMPTS} attempt(s), \
                             keeping {} valid item(s)/{} valid document(s), dropped {} malformed entrie(s)",
                            cleaned.get("knowledge_items").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
                            cleaned.get("documents").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
                            errors.len()
                        ),
                    );
                    chosen = Some(cleaned);
                }
            }

            match chosen {
                Some(r) => (Some(r), None),
                None => {
                    let reason = last_err.map(|e| e.to_string()).unwrap_or_else(|| "author() never returned a response".to_string());
                    warn_section(bundle_dir, format!("[sopkb.mine] section {section_label}: skipped after {MAX_AUTHOR_ATTEMPTS} failed attempt(s): {reason}"));
                    (None, Some(reason))
                }
            }
        })();

        let done = completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        if let Some(cb) = on_progress {
            cb(done, total);
        }
        outcome
    });

    // Step 2: apply already-fetched responses in original section order,
    // sequentially -- see this function's own doc comment for why this half
    // cannot be parallelized.
    let mut items: Vec<Value> = Vec::new();
    let mut writes: Vec<Value> = Vec::new();
    let mut staged_docs: Vec<PreparedDoc> = Vec::new();
    let mut ordinal: u32 = 1;

    for (section, (response, _reason)) in sections.iter().zip(responses) {
        let Some(response) = response else { continue };
        let section_label = section.get("id").and_then(|v| v.as_str()).unwrap_or("<unknown>").to_string();

        let result: Result<()> = (|| {
            let (section_items, section_writes, section_staged, next_ordinal) =
                apply_author_response(bundle_dir, section, &response, ordinal, actor)?;
            items.extend(section_items);
            writes.extend(section_writes);
            staged_docs.extend(section_staged);
            ordinal = next_ordinal;
            Ok(())
        })();
        if let Err(e) = result {
            warn_section(bundle_dir, format!("[sopkb.mine] section {section_label}: skipped after apply failure: {e}"));
        }
    }

    for prepared in &staged_docs {
        okf_writer::write_prepared_doc(&okf_root, prepared)?;
    }

    // Carry forward / supersede prior versions' items before anything derived from the
    // item list is computed -- entities and triples must cover the MERGED list, not
    // just this pass's output, or a superseded item would lose its graph presence.
    let items: Vec<Value> = sopkb_core::knowledge_lifecycle::merge_mined_items(&existing_items, &items, &sections);

    let mut entities: OrderedMap<Value> = OrderedMap::new();
    for item in &items {
        let subject = item.get("subject").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        if !entities.contains_key(&subject) {
            entities.insert(subject.clone(), json!({ "id": format!("entity-{}", sopkb_core::ids::slugify(&subject)), "label": subject }));
        }
    }

    sopkb_core::store::write_state_json(bundle_dir, "items.json", &Value::Array(items.clone()))?;
    let entities_list: Vec<Value> = entities.iter().map(|(_, v)| v.clone()).collect();
    sopkb_core::store::write_state_json(bundle_dir, "entities.json", &Value::Array(entities_list))?;
    sopkb_core::store::write_state_json(bundle_dir, "llm_authoring.json", &json!({ "actor": actor, "writes": writes }))?;
    let triples: Vec<Value> = items
        .iter()
        .map(|it| {
            json!({
                "id": format!("triple-{}", it.get("id").and_then(|v| v.as_str()).unwrap_or_default()),
                "knowledge_item_id": it["id"],
                "subject": it["subject"],
                "predicate": it["predicate"],
                "object": it["object"],
            })
        })
        .collect();
    sopkb_core::store::write_state_json(bundle_dir, "triples.json", &Value::Array(triples))?;

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use sopkb_llm::MockTransport;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::tempdir;

    fn simple_sop_bundle() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        let sources = dir.path().join("sources");
        fs::create_dir_all(&sources).unwrap();
        fs::write(
            sources.join("glp1_intake.md"),
            "# GLP-1 Intake SOP\n\n## Purpose\n\nThis SOP defines intake checks for GLP-1 therapy requests.\n\n## Eligibility Requirements\n\nClinicians must confirm patient identity before reviewing therapy eligibility.\n\n## Procedure\n\nStaff should record contraindications and route uncertain cases for clinical review.\n",
        )
        .unwrap();
        let bundle_dir = dir.path().join("bundle");
        sopkb_core::store::create_bundle(&bundle_dir, Some("LLM Author Test")).unwrap();
        sopkb_core::inventory::scan_sources(&sources, &bundle_dir).unwrap();
        sopkb_core::normalize::normalize_sources(&bundle_dir, None, None).unwrap();
        dir
    }

    #[test]
    fn strip_markdown_fence_removes_json_language_tagged_fence() {
        let text = "```json\n{\"documents\": []}\n```";
        assert_eq!(strip_markdown_fence(text), "{\"documents\": []}");
    }

    #[test]
    fn strip_markdown_fence_removes_bare_fence() {
        let text = "```\n{\"documents\": []}\n```";
        assert_eq!(strip_markdown_fence(text), "{\"documents\": []}");
    }

    #[test]
    fn strip_markdown_fence_leaves_unfenced_text_untouched() {
        let text = "{\"documents\": []}";
        assert_eq!(strip_markdown_fence(text), text);
    }

    #[test]
    fn parse_author_response_tolerates_fence_p_m12() {
        let fenced = "```json\n{\"documents\": [], \"knowledge_items\": []}\n```";
        let parsed = parse_author_response(fenced).unwrap();
        assert!(parsed.get("documents").is_some());
    }

    #[test]
    fn parse_author_response_rejects_non_object_and_bad_json() {
        assert!(parse_author_response("[1,2,3]").is_err());
        assert!(parse_author_response("not json").is_err());
    }

    #[test]
    fn build_section_author_request_p_m15_reads_original_path_not_path() {
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");
        let sections = sopkb_core::store::read_state_json(&bundle_dir, "sections.json", json!([])).unwrap();
        let section = sections.as_array().unwrap().iter().find(|s| s["heading"] == "Procedure").unwrap();
        let request = build_section_author_request(&bundle_dir, section).unwrap();
        let resource = request["source"]["resource"].as_str().unwrap();
        assert!(resource.starts_with("sources/originals/"), "expected original_path-derived resource, got {resource}");
    }

    #[test]
    fn build_author_messages_user_content_is_canonical_json_p_m16() {
        let request = json!({"b": 1, "a": {"z": 1, "y": 2}});
        let messages = build_author_messages(&request, None, None);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, AUTHOR_SYSTEM_PROMPT);
        assert_eq!(messages[1].role, "user");
        let expected_json = sopkb_fmt::to_canonical_json(&request);
        assert!(messages[1].content.ends_with(&expected_json));
        assert!(messages[1].content.starts_with("Author OKF SOP knowledge for this normalized source section."));
    }

    #[test]
    fn validate_author_response_drops_malformed_knowledge_item_keeps_valid_ones() {
        let response = parse_author_response(
            r#"{"knowledge_items": [
                {"subject": "s1", "predicate": "p1", "object": "o1", "source_text": "t1"},
                {"subject": "s2"},
                {"subject": "s3", "predicate": "p3", "object": "o3", "source_text": "t3"}
            ]}"#,
        )
        .unwrap();
        let (cleaned, errors) = validate_author_response(&response);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("knowledge item dropped"), "{errors:?}");
        let items = cleaned.get("knowledge_items").and_then(|v| v.as_array()).unwrap();
        assert_eq!(items.len(), 2, "the one malformed candidate must be dropped, the two valid ones kept");
    }

    #[test]
    fn validate_author_response_falsy_fields_coerce_to_empty_with_no_errors() {
        let response = parse_author_response(r#"{"documents": false, "knowledge_items": 0}"#).unwrap();
        let (cleaned, errors) = validate_author_response(&response);
        assert!(errors.is_empty());
        assert_eq!(cleaned.get("documents").and_then(|v| v.as_array()).unwrap().len(), 0);
        assert_eq!(cleaned.get("knowledge_items").and_then(|v| v.as_array()).unwrap().len(), 0);
    }

    #[test]
    fn mine_with_author_retries_and_recovers_before_exhausting_attempts() {
        // First attempt returns a malformed candidate; second attempt (simulating a
        // model non-determinism retry, same as real oss-launch's rationale) returns a
        // clean response. The recovered response must be used, not thrown away.
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");
        // `AtomicU32`, not `Cell`: `author` is now called from a worker-pool
        // closure that must be `Sync` (see `mine_with_author`'s `F: ... + Sync`
        // bound), and `Cell` is never `Sync` even behind a shared `&self` call.
        let attempts = std::sync::atomic::AtomicU32::new(0);
        let author = |request: &Value| -> Result<OrderedMap<OrderedJson>> {
            let heading = request["section"]["heading"].as_str().unwrap_or_default();
            if heading != "Procedure" {
                return parse_author_response(r#"{"documents": [], "knowledge_items": []}"#);
            }
            let attempt = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if attempt < 2 {
                parse_author_response(r#"{"knowledge_items": [{"subject": "s"}]}"#)
            } else {
                parse_author_response(
                    r#"{"knowledge_items": [{"subject": "Procedure", "predicate": "should", "object": "o", "source_text": "Staff should record contraindications and route uncertain cases for clinical review."}]}"#,
                )
            }
        };
        let items = mine_with_author(&bundle_dir, author, "test/llm-author", None, None).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["subject"], "Procedure");
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn mine_with_author_actually_runs_sections_concurrently_not_just_compiles() {
        // `simple_sop_bundle` has 4 sections. Each `author()` call sleeps briefly
        // while recording how many calls are in flight at once -- if Step 1 were
        // still secretly sequential (e.g. a `+ Sync` bound satisfied but
        // `parallel_map` not actually reached), max-observed-concurrency would be
        // 1, not >1. This is the one test in this module that would catch a
        // regression back to sequential processing even though the type
        // signature alone can't.
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");
        let in_flight = std::sync::atomic::AtomicUsize::new(0);
        let max_observed = std::sync::atomic::AtomicUsize::new(0);
        let author = |_: &Value| -> Result<OrderedMap<OrderedJson>> {
            let now = in_flight.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            max_observed.fetch_max(now, std::sync::atomic::Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(30));
            in_flight.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            parse_author_response(r#"{"documents": [], "knowledge_items": []}"#)
        };
        mine_with_author(&bundle_dir, author, "test/llm-author", None, None).unwrap();
        assert!(
            max_observed.load(std::sync::atomic::Ordering::SeqCst) > 1,
            "expected multiple sections' author() calls in flight at once, saw max {}",
            max_observed.load(std::sync::atomic::Ordering::SeqCst)
        );
    }

    #[test]
    fn mine_with_author_on_progress_reaches_total_and_never_double_counts() {
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");
        let author = |_: &Value| -> Result<OrderedMap<OrderedJson>> {
            parse_author_response(r#"{"documents": [], "knowledge_items": []}"#)
        };
        let calls: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());
        let on_progress = |done: usize, total: usize| calls.lock().unwrap().push((done, total));
        mine_with_author(&bundle_dir, author, "test/llm-author", Some(&on_progress), None).unwrap();

        let calls = calls.into_inner().unwrap();
        assert_eq!(calls.len(), 4, "one progress call per section, exactly once each");
        assert!(calls.iter().all(|&(_, total)| total == 4));
        let mut done_values: Vec<usize> = calls.iter().map(|&(done, _)| done).collect();
        done_values.sort_unstable();
        assert_eq!(done_values, vec![1, 2, 3, 4], "done values must be exactly 1..=total with no gaps or repeats");
    }

    #[test]
    fn mine_with_author_is_cancelled_true_from_the_start_skips_every_section() {
        // `is_cancelled` returning true BEFORE any section starts must skip every
        // section without ever calling `author()` -- the whole point of checking
        // before dispatch, not mid-flight.
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");
        let author_calls = std::sync::atomic::AtomicU32::new(0);
        let author = |_: &Value| -> Result<OrderedMap<OrderedJson>> {
            author_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            parse_author_response(r#"{"documents": [], "knowledge_items": []}"#)
        };
        let is_cancelled = || true;
        let items = mine_with_author(&bundle_dir, author, "test/llm-author", None, Some(&is_cancelled)).unwrap();
        assert!(items.is_empty());
        assert_eq!(author_calls.load(std::sync::atomic::Ordering::SeqCst), 0, "author() must never be called once cancelled");
    }

    #[test]
    fn mine_with_author_run_never_fails_outright_when_cancelled_mid_flight() {
        // A run cancelled partway through still returns Ok (possibly with fewer
        // items than an uncancelled run would produce) -- cancellation is a
        // graceful early stop, not a new error path.
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");
        let seen = std::sync::atomic::AtomicU32::new(0);
        let author = |_: &Value| -> Result<OrderedMap<OrderedJson>> {
            seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            parse_author_response(r#"{"documents": [], "knowledge_items": []}"#)
        };
        // Cancelled after the first section is already claimed by a worker.
        let is_cancelled = || seen.load(std::sync::atomic::Ordering::SeqCst) >= 1;
        let result = mine_with_author(&bundle_dir, author, "test/llm-author", None, Some(&is_cancelled));
        assert!(result.is_ok(), "cancellation must not surface as an Err");
    }

    #[test]
    fn mine_with_author_writes_validated_okf_and_authored_items() {
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");

        let author = |request: &Value| -> Result<OrderedMap<OrderedJson>> {
            let heading = request["section"]["heading"].as_str().unwrap_or_default();
            let body = if heading == "Procedure" {
                r##"{
                  "documents": [
                    {
                      "concept_id": "rules/route-uncertain-case",
                      "frontmatter": {
                        "type": "SOP Decision Rule",
                        "title": "Route uncertain cases for clinical review",
                        "sources": [{"id": "src", "resource": "../sources/source.md", "title": "Source"}],
                        "sopkb": {"rule": {"id": "rule-authored-route-uncertain-case",
                          "condition": {"fact": "case_uncertainty", "operator": "is_true", "label": "Case is uncertain"},
                          "obligation": {"fact": "route_clinical_review", "action": "route", "label": "Route case for clinical review"},
                          "otherwise": {"action_required": false, "label": "Not required for certain cases."}}}
                      },
                      "body": "# Route uncertain cases for clinical review\n"
                    }
                  ],
                  "knowledge_items": [
                    {
                      "subject": "Procedure",
                      "predicate": "should",
                      "object": "Staff should record contraindications and route uncertain cases for clinical review.",
                      "source_text": "Staff should record contraindications and route uncertain cases for clinical review.",
                      "decision_rules": [{"id": "rule-authored-route-uncertain-case", "title": "Route uncertain cases"}]
                    }
                  ]
                }"##
            } else {
                r#"{"documents": [], "knowledge_items": []}"#
            };
            parse_author_response(body)
        };

        let items = mine_with_author(&bundle_dir, author, "test/llm-author", None, None).unwrap();
        assert_eq!(items.len(), 1);
        assert!(bundle_dir.join(".sopkb").join("authored_okf").join("rules").join("route-uncertain-case.md").exists());
        let items_on_disk = sopkb_core::store::read_state_json(&bundle_dir, "items.json", json!([])).unwrap();
        assert_eq!(items_on_disk.as_array().unwrap().len(), 1);
        assert_eq!(items_on_disk[0]["derivation"], "llm_authored");
        assert_eq!(items_on_disk[0]["metadata"]["decision_rules"][0]["id"], "rule-authored-route-uncertain-case");
        assert_eq!(items_on_disk[0]["metadata"]["provider"], "test/llm-author");
    }

    #[test]
    fn mine_with_author_skips_failed_section_without_discarding_others() {
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");

        // Sections are processed in document order: "GLP-1 Intake SOP", "Purpose",
        // "Eligibility Requirements", "Procedure". "Eligibility Requirements" (3rd)
        // succeeds; "Procedure" (4th) fails on every attempt (never valid JSON, so
        // retries can't save it either). This is the exact scenario a real user hit
        // ("mining failed for 1 of 1 section(s), nothing written") once
        // heading-restructuring started producing many more, smaller sections -- one
        // section failing must not discard every other section's mined output.
        let author = |request: &Value| -> Result<OrderedMap<OrderedJson>> {
            let heading = request["section"]["heading"].as_str().unwrap_or_default();
            if heading == "Eligibility Requirements" {
                parse_author_response(
                    r##"{"documents": [{"concept_id": "notes/would-be-written", "frontmatter": {"type": "Note"}, "body": "# hi\n"}], "knowledge_items": []}"##,
                )
            } else if heading == "Procedure" {
                parse_author_response("not json at all")
            } else {
                parse_author_response(r#"{"documents": [], "knowledge_items": []}"#)
            }
        };

        let items = mine_with_author(&bundle_dir, author, "test/llm-author", None, None).unwrap();
        assert!(items.is_empty(), "Eligibility Requirements' response had no knowledge_items");

        // The successful section's document must land on disk even though a later
        // section failed on every retry attempt.
        assert!(bundle_dir.join(".sopkb").join("authored_okf").join("notes").join("would-be-written.md").exists());
        assert!(bundle_dir.join(".sopkb").join("items.json").exists());
        assert!(bundle_dir.join(".sopkb").join("llm_authoring.json").exists());
    }

    #[test]
    fn mine_with_author_apply_response_falsy_coercion_p_m19() {
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");
        let author =
            |_: &Value| -> Result<OrderedMap<OrderedJson>> { parse_author_response(r#"{"documents": false, "knowledge_items": 0}"#) };
        let items = mine_with_author(&bundle_dir, author, "test/llm-author", None, None).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn mine_with_author_apply_response_truthy_non_list_is_dropped_not_fatal_p_m19() {
        // A response shaped this badly can't be repaired by validate_author_response
        // (it returns the response unmodified, per Python parity) or by retrying (the
        // closure is deterministic) -- every section exhausts its attempts and is
        // skipped, but the run as a whole still succeeds with no items, matching real
        // oss-launch's graceful-degradation behavior rather than failing outright.
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");
        let author = |_: &Value| -> Result<OrderedMap<OrderedJson>> { parse_author_response(r#"{"documents": "oops"}"#) };
        let items = mine_with_author(&bundle_dir, author, "test/llm-author", None, None).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn mine_with_author_non_mapping_document_element_is_dropped_not_a_panic() {
        // The one malformed document entry is dropped by validate_author_response
        // (not the whole section); with no valid documents or items left, the section
        // applies cleanly as a no-op rather than failing.
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");
        let author = |_: &Value| -> Result<OrderedMap<OrderedJson>> { parse_author_response(r#"{"documents": ["oops"]}"#) };
        let items = mine_with_author(&bundle_dir, author, "test/llm-author", None, None).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn authored_confidence_zero_silently_becomes_default_0_7() {
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");
        let author = |request: &Value| -> Result<OrderedMap<OrderedJson>> {
            if request["section"]["heading"] == "Procedure" {
                parse_author_response(
                    r#"{"documents": [], "knowledge_items": [{"subject": "s", "predicate": "p", "object": "o", "source_text": "Staff should record contraindications and route uncertain cases for clinical review.", "confidence": 0}]}"#,
                )
            } else {
                parse_author_response(r#"{"documents": [], "knowledge_items": []}"#)
            }
        };
        let items = mine_with_author(&bundle_dir, author, "test/llm-author", None, None).unwrap();
        assert_eq!(items[0]["confidence"], 0.7);
    }

    /// End-to-end plumbing test through the real HTTP-shaped path: request building,
    /// `sopkb_llm::author_call_with_transport`, response parsing, and document writing,
    /// using `MockTransport` so no live LLM call is made. Also proves the outbound
    /// request body byte-matches `json_dumps(request, indent=2, sort_keys=True)`
    /// (P-M16/P-M17).
    #[test]
    #[serial]
    fn azure_llm_author_with_transport_end_to_end_via_mock() {
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");
        with_test_profile(&dir, || {
            let sections = sopkb_core::store::read_state_json(&bundle_dir, "sections.json", json!([])).unwrap();
            let section = sections.as_array().unwrap().iter().find(|s| s["heading"] == "Procedure").unwrap().clone();
            let request = build_section_author_request(&bundle_dir, &section).unwrap();

            let canned_body = fs::read_to_string(repo_root().join("v2/sopkb-rust/fixtures/llm-responses/mining-sample.json")).unwrap();
            let response_payload = format!(r#"{{"output_text": {}}}"#, sopkb_fmt::to_canonical_json(&Value::String(canned_body)));
            let transport = MockTransport::ok(200, response_payload.into_bytes());

            let response = azure_llm_author_with_transport(&request, None, None, &transport).unwrap();
            assert!(response.get("documents").is_some());

            let sent = transport.last_request().unwrap();
            let sent_body: Value = serde_json::from_str(&String::from_utf8(sent.body).unwrap()).unwrap();
            let expected_input = format!(
                "Author OKF SOP knowledge for this normalized source section. Return only JSON matching the required shape.\n\n{}",
                sopkb_fmt::to_canonical_json(&request)
            );
            assert_eq!(sent_body["input"], expected_input, "outbound request body must match json_dumps(request, indent=2, sort_keys=True)");
            assert_eq!(sent_body["instructions"], AUTHOR_SYSTEM_PROMPT);
        });
    }

    #[test]
    #[serial]
    fn mine_bundle_azure_llm_provider_via_mock_transport_writes_authored_items() {
        let dir = simple_sop_bundle();
        let bundle_dir = dir.path().join("bundle");
        with_test_profile(&dir, || {
            let canned_body = fs::read_to_string(repo_root().join("v2/sopkb-rust/fixtures/llm-responses/mining-sample.json")).unwrap();

            // Only the "Procedure" section is routed through the real MockTransport
            // plumbing (proving the HTTP-shaped path end-to-end via `mine_bundle`'s
            // azure-llm dispatch); the others return empty responses directly, since
            // `MockTransport` is single-shot and this test's focus is the dispatch +
            // one representative section, not every section.
            let author = |request: &Value| -> Result<OrderedMap<OrderedJson>> {
                if request["section"]["heading"] == "Procedure" {
                    let payload = format!(r#"{{"output_text": {}}}"#, sopkb_fmt::to_canonical_json(&Value::String(canned_body.clone())));
                    let transport = MockTransport::ok(200, payload.into_bytes());
                    azure_llm_author_with_transport(request, None, None, &transport)
                } else {
                    parse_author_response(r#"{"documents": [], "knowledge_items": []}"#)
                }
            };

            let items = mine_with_author(&bundle_dir, author, "sopkb/azure-llm", None, None).unwrap();
            assert_eq!(items.len(), 1);
            assert_eq!(items[0]["subject"], "Procedure");

            let llm_authoring = sopkb_core::store::read_state_json(&bundle_dir, "llm_authoring.json", json!({})).unwrap();
            assert_eq!(llm_authoring["actor"], "sopkb/azure-llm");
            assert_eq!(llm_authoring["writes"].as_array().unwrap().len(), 1);
        });
    }

    fn repo_root() -> std::path::PathBuf {
        // Walk up from this crate's own directory until a top-level `.git` entry is
        // found (a directory in a normal checkout, a file in a git worktree) --
        // depth-agnostic, unlike a fixed `ancestors().nth(N)` hop count.
        let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        loop {
            if dir.join(".git").exists() {
                return dir;
            }
            dir = dir.parent().expect("repo_root: reached filesystem root without finding a .git entry").to_path_buf();
        }
    }

    /// Points `SOPKB_SETTINGS_PATH` at a fresh temp file, saves a usable profile, runs
    /// `f`, then clears the env var -- mirrors the `with_settings_path` helper already
    /// used by `sopkb-config`/`sopkb-llm`'s own tests. Callers must be `#[serial]`
    /// (env vars are process-global).
    fn with_test_profile<F: FnOnce()>(dir: &tempfile::TempDir, f: F) {
        let settings_path = dir.path().join("settings.json");
        unsafe { std::env::set_var("SOPKB_SETTINGS_PATH", &settings_path) };
        let profile = sopkb_config::ModelProfile {
            id: "p1".into(),
            name: "One".into(),
            base_url: "https://example.test".into(),
            api_key: "secret".into(),
            model: "gpt-x".into(),
            ..Default::default()
        };
        sopkb_config::save_profile(&profile).unwrap();
        f();
        unsafe { std::env::remove_var("SOPKB_SETTINGS_PATH") };
    }
}
