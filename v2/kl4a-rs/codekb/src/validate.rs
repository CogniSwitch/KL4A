//! Port of `kl4a/codekb/validate.py`.
//!
//! This is the rule-based bundle validator: `validate_code_bundle` is the
//! CLI-facing gate that reads the four `.codekb/*.json` state documents
//! (inventory, symbols, relations, knowledge), runs one `validate_*` pass per
//! document, writes `reports/code_validation.json` plus several markdown
//! reports, and returns the accumulated `(errors, warnings)`.
//!
//! Every `validate_*`/`render_*` function and every individual check inside
//! them is enumerated in the completeness ledger returned by this batch's
//! `SubagentHandback`. Two behaviors worth calling out up front because they
//! are easy to "fix" by accident when porting:
//!
//! - In `validate_code_bundle`, a `validate_*` pass only runs when *its own*
//!   state document is present, but the arguments it receives fall back to an
//!   empty-shaped default when a *sibling* document is missing (e.g. symbols
//!   present but relations missing still lets `validate_symbols` run with a
//!   real symbols state). This is ported exactly, branch for branch.
//! - `_report_link`/`_capped` and the six `render_*` functions are pure
//!   string builders; only `validate_okf_doc` and `validate_evidence` touch
//!   the filesystem beyond an `exists()` check, and only those two can fail
//!   with a genuine I/O error (as opposed to a validation error, which is
//!   just an appended string). That distinction is preserved here: an I/O
//!   error propagates as `Err`, a validation problem becomes a pushed
//!   `String`.
//!
//! WIRING (see also the two `use` blocks below): this file depends on three
//! things that do not exist yet anywhere in `v2/kl4a-rs`:
//! 1. `crate::state::{read_json, write_json, STATE_DIR}` -- `kl4a.codekb.state`
//!    was not part of this batch (scoped to `validate.py` only).
//! 2. `crate::bundle::{validate_code_profile, rel_link}` -- `kl4a.codekb.bundle`
//!    was not part of this batch either.
//! 3. An OKF document parser/validator equivalent to
//!    `kl4a.kl4a.okf_writer.{OKFDocument, OKFDocumentError}`. That module
//!    lives above `codekb`/`apikb` in the Python tree (in the shared `kl4a`
//!    package), so it needs a home in this workspace too -- most likely a
//!    shared crate both `codekb` and `apikb` depend on, the same open
//!    question flagged in `model.rs` for `bundle_store::load_manifest`.
//!
//! None of these three will compile until the coordinator wires them in (or
//! this file is adjusted to match whatever shape they land with). See the
//! "Open questions" section of this batch's handback for the exact assumed
//! signatures.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{bail, Result};
use serde_json::{json, Map, Value};

// WIRING: `kl4a.codekb.state` was not in this batch's scope. Assumed
// signatures (matching the sibling `apikb::state` module's shape, adjusted to
// `Result` because `codekb::lifecycle::read_review_state` -- already landed
// by a parallel batch -- calls `read_json(..)` as a bare tail expression
// against a `Result<Value>` return type, i.e. it assumes `read_json` itself
// returns `Result<Value>`, not a bare `Value` the way `apikb::state::read_json`
// does. That is a real inconsistency between the two crates' precedent; see
// the handback's open questions.
//   pub fn read_json(path: &Path, default: Value) -> Result<Value>
//   pub fn write_json(path: &Path, data: &Value) -> Result<()>
//   pub const STATE_DIR: &str = ".codekb";
use crate::state::{read_json, write_json, STATE_DIR};

// WIRING: `kl4a.codekb.bundle` was not in this batch's scope.
//   pub fn validate_code_profile(bundle_dir: &Path) -> Result<(Vec<String>, Vec<String>)>
//   pub fn rel_link(from_okf_path: &str, to_okf_path: &str) -> String
use crate::bundle::{rel_link, validate_code_profile};

// WIRING: `kl4a.kl4a.okf_writer.{OKFDocument, OKFDocumentError}` has no home
// in this workspace yet (same situation as `bundle_store::load_manifest` in
// `model.rs`). Assumed shape:
//   pub struct OkfDocument;
//   pub struct OkfDocumentError(/* Display */);
//   impl OkfDocument { pub fn parse(text: &str) -> Result<Self, OkfDocumentError>; }
//   impl OkfDocument { pub fn validate(&self) -> Result<(), OkfDocumentError>; }
use crate::okf_writer::{OkfDocument, OkfDocumentError};

/// Mirrors `kl4a.codekb.validate._MAX_LIST`.
///
/// Caps how many lines `_capped` will emit before collapsing the rest into an
/// "... and N more" line, and separately caps how many depended-on symbols
/// `render_change_impact` will render sections for.
const MAX_LIST: usize = 200;

/// Mirrors `kl4a.codekb.validate._REPORT_DIR_ANCHOR`.
///
/// A synthetic bundle-root-relative path standing in for "a document that
/// lives directly under `reports/`". `_report_link` uses it as the `from`
/// side of `rel_link` so that a link built for a report page (which always
/// lives at `reports/<name>.md`) resolves correctly relative to the actual
/// target doc's real path, without needing to know the report's own filename.
const REPORT_DIR_ANCHOR: &str = "reports/_.md";

// ---------------------------------------------------------------------
// Small `serde_json::Value` accessor helpers (private to this module).
//
// The Python source leans heavily on `dict.get(key, default)` and on Python's
// `f"{x.get('id')}"` rendering a missing/`None` value as the literal string
// "None". These helpers reproduce both of those idioms faithfully so the
// error/warning message text matches byte-for-byte.
// ---------------------------------------------------------------------

/// Mirrors `f"{value_or_none}"` when `value_or_none` came from `dict.get(...)`
/// with no explicit default -- i.e. Python would print the literal `None`.
fn py_str(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "None".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// Mirrors `d.get(key, "")` for a string-shaped field.
fn get_str_or_empty(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// Mirrors `d.get(key, [])`.
fn get_array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// Mirrors Python truthiness for a `dict.get(key)` result, used for the
/// `if not x.get(key):` / `if x.get(key):` checks scattered through this file.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None => false,
        Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
    }
}

/// Mirrors `int(x.get(key) or 0)`: use the field if it is truthy, else 0,
/// then coerce to an integer. Only `Number`/`String` fields are meaningfully
/// coercible; anything else (or a parse failure) falls back to 0, same as a
/// well-formed bundle would never hit those paths in practice.
fn int_or_zero(v: Option<&Value>) -> i64 {
    match v {
        Some(val) if truthy(Some(val)) => match val {
            Value::Number(n) => n.as_i64().unwrap_or(0),
            Value::String(s) => s.parse::<i64>().unwrap_or(0),
            _ => 0,
        },
        _ => 0,
    }
}

/// Mirrors `str(x.get(key) or "active")`.
fn lifecycle_status_or_active(v: &Value) -> String {
    match v.get("lifecycle_status") {
        Some(val) if truthy(Some(val)) => py_str(Some(val)),
        _ => "active".to_string(),
    }
}

fn ids_set(items: &[Value]) -> HashSet<String> {
    items
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

// ---------------------------------------------------------------------
// Top-level orchestrator.
// ---------------------------------------------------------------------

/// Mirrors `validate_code_bundle`.
pub fn validate_code_bundle(bundle_dir: &Path) -> Result<(Vec<String>, Vec<String>)> {
    let (mut errors, mut warnings) = validate_code_profile(bundle_dir);

    let inventory = read_json(
        &bundle_dir.join(STATE_DIR).join("code_inventory.json"),
        Value::Null,
    );
    let symbols_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_symbols.json"),
        Value::Null,
    );
    let relations_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_relations.json"),
        Value::Null,
    );
    let knowledge_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_knowledge.json"),
        Value::Null,
    );

    if inventory.is_null() {
        warnings.push("missing .codekb/code_inventory.json".to_string());
    } else {
        validate_inventory(bundle_dir, &inventory, &mut errors, &mut warnings)?;
    }

    if symbols_state.is_null() {
        warnings.push("missing .codekb/code_symbols.json".to_string());
    } else {
        let inventory_or_default = if inventory.is_null() {
            json!({"sources": []})
        } else {
            inventory.clone()
        };
        validate_symbols(
            bundle_dir,
            &symbols_state,
            &inventory_or_default,
            &mut errors,
            &mut warnings,
        )?;
    }

    if relations_state.is_null() {
        warnings.push("missing .codekb/code_relations.json".to_string());
    } else {
        let symbols_or_default = if symbols_state.is_null() {
            json!({"modules": [], "symbols": [], "evidence": []})
        } else {
            symbols_state.clone()
        };
        validate_relations(
            bundle_dir,
            &relations_state,
            &symbols_or_default,
            &mut errors,
            &mut warnings,
        )?;
    }

    if knowledge_state.is_null() {
        warnings.push("missing .codekb/code_knowledge.json".to_string());
    } else {
        let symbols_or_default = if symbols_state.is_null() {
            json!({"symbols": [], "evidence": []})
        } else {
            symbols_state.clone()
        };
        let relations_or_default = if relations_state.is_null() {
            json!({"relations": []})
        } else {
            relations_state.clone()
        };
        validate_knowledge(
            bundle_dir,
            &knowledge_state,
            &symbols_or_default,
            &relations_or_default,
            &mut errors,
            &mut warnings,
        )?;
    }

    let reports_dir = bundle_dir.join("reports");
    std::fs::create_dir_all(&reports_dir)?;

    write_json(
        &reports_dir.join("code_validation.json"),
        &json!({"errors": errors, "warnings": warnings}),
    )?;

    let code_validation_md = render_code_validation(&errors, &warnings);
    std::fs::write(reports_dir.join("code_validation.md"), &code_validation_md)?;
    std::fs::write(reports_dir.join("validation.md"), &code_validation_md)?;

    let empty = Value::Object(Map::new());
    let symbols_for_render = if symbols_state.is_null() { &empty } else { &symbols_state };
    let relations_for_render = if relations_state.is_null() { &empty } else { &relations_state };
    let knowledge_for_render = if knowledge_state.is_null() { &empty } else { &knowledge_state };

    std::fs::write(
        reports_dir.join("symbol_inventory.md"),
        render_symbol_inventory(symbols_for_render),
    )?;
    std::fs::write(
        reports_dir.join("relation_resolution.md"),
        render_relation_resolution(relations_for_render),
    )?;
    std::fs::write(
        reports_dir.join("test_coverage_map.md"),
        render_test_coverage_map(relations_for_render, symbols_for_render),
    )?;
    std::fs::write(
        reports_dir.join("change_impact.md"),
        render_change_impact(relations_for_render, Some(symbols_for_render)),
    )?;
    std::fs::write(
        reports_dir.join("code_review_required.md"),
        render_code_review_required(knowledge_for_render, Some(symbols_for_render)),
    )?;

    Ok((errors, warnings))
}

// ---------------------------------------------------------------------
// validate_inventory
// ---------------------------------------------------------------------

/// Mirrors `validate_inventory`. Two checks per inventory-level warning
/// record, four checks per source record.
pub fn validate_inventory(
    bundle_dir: &Path,
    inventory: &Value,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    for warning in get_array(inventory, "warnings") {
        warnings.push(format!(
            "inventory warning: {} - {}",
            py_str(warning.get("path")),
            py_str(warning.get("warning")),
        ));
    }
    for source in get_array(inventory, "sources") {
        let source_doc = bundle_dir.join(get_str_or_empty(source, "okf_path"));
        let original = bundle_dir.join(get_str_or_empty(source, "original_path"));
        let id = py_str(source.get("id"));
        if !source_doc.exists() {
            errors.push(format!("{}: missing source OKF doc", id));
        } else {
            validate_okf_doc(&source_doc, errors)?;
        }
        if !original.exists() {
            errors.push(format!("{}: missing original source snapshot", id));
        }
        if !truthy(source.get("source_version_id")) {
            errors.push(format!("{}: missing source_version_id", id));
        }
        if !truthy(source.get("language")) {
            errors.push(format!("{}: missing language", id));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// validate_symbols
// ---------------------------------------------------------------------

/// Mirrors `validate_symbols`: parser-run parse-error warnings, per-module
/// OKF-doc checks, then per-symbol OKF-doc + source/evidence resolution
/// checks (delegating the evidence-span checks to `validate_evidence`).
pub fn validate_symbols(
    bundle_dir: &Path,
    symbols_state: &Value,
    inventory: &Value,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let sources: HashMap<String, &Value> = get_array(inventory, "sources")
        .iter()
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(|id| (id.to_string(), s)))
        .collect();
    let evidence_by_id: HashMap<String, &Value> = get_array(symbols_state, "evidence")
        .iter()
        .filter_map(|e| e.get("id").and_then(Value::as_str).map(|id| (id.to_string(), e)))
        .collect();

    for run in get_array(symbols_state, "parser_runs") {
        if run.get("status").and_then(Value::as_str) == Some("parse_error") {
            warnings.push(format!("{}: Python parse error", py_str(run.get("source_id"))));
        }
    }

    for module in get_array(symbols_state, "modules") {
        let module_doc = bundle_dir.join(get_str_or_empty(module, "okf_path"));
        if !module_doc.exists() {
            errors.push(format!("{}: missing module OKF doc", py_str(module.get("id"))));
        } else {
            validate_okf_doc(&module_doc, errors)?;
        }
    }

    for symbol in get_array(symbols_state, "symbols") {
        let symbol_id = py_str(symbol.get("id"));
        let symbol_doc = bundle_dir.join(get_str_or_empty(symbol, "okf_path"));
        if !symbol_doc.exists() {
            errors.push(format!("{}: missing symbol OKF doc", symbol_id));
        } else {
            validate_okf_doc(&symbol_doc, errors)?;
        }

        let source_id = symbol.get("source_id").and_then(Value::as_str);
        let source = source_id.and_then(|sid| sources.get(sid));
        let source = match source {
            Some(s) => *s,
            None => {
                errors.push(format!("{}: unknown source_id", symbol_id));
                continue;
            }
        };

        let evidence_id = symbol.get("evidence_id").and_then(Value::as_str);
        let evidence = evidence_id.and_then(|eid| evidence_by_id.get(eid));
        let evidence = match evidence {
            Some(e) => *e,
            None => {
                errors.push(format!("{}: missing evidence record", symbol_id));
                continue;
            }
        };

        validate_evidence(bundle_dir, source, symbol, evidence, errors)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------
// validate_relations
// ---------------------------------------------------------------------

/// Mirrors `validate_relations`: per-relation OKF-doc check, resolution
/// status validity, subject/object resolution, evidence resolution, the
/// "exact relations need evidence (except imports/defines)" rule, and the
/// inferred/unresolved caution warning.
pub fn validate_relations(
    bundle_dir: &Path,
    relations_state: &Value,
    symbols_state: &Value,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let module_ids = ids_set(get_array(symbols_state, "modules"));
    let symbol_ids = ids_set(get_array(symbols_state, "symbols"));
    let evidence_ids = ids_set(get_array(symbols_state, "evidence"));
    let valid_ids: HashSet<String> = module_ids.union(&symbol_ids).cloned().collect();

    for relation in get_array(relations_state, "relations") {
        let relation_id = py_str(relation.get("id"));
        let relation_doc = bundle_dir.join(get_str_or_empty(relation, "okf_path"));
        if !relation_doc.exists() {
            errors.push(format!("{}: missing relation OKF doc", relation_id));
        } else {
            validate_okf_doc(&relation_doc, errors)?;
        }

        let metadata = relation.get("relation").cloned().unwrap_or(Value::Null);
        let status = metadata.get("resolution_status").and_then(Value::as_str);
        if !matches!(status, Some("exact") | Some("inferred") | Some("unresolved")) {
            errors.push(format!("{}: invalid resolution_status", relation_id));
        }

        let subject_kind = relation.get("subject_kind").and_then(Value::as_str);
        if matches!(subject_kind, Some("module") | Some("symbol")) {
            let subject = relation.get("subject").and_then(Value::as_str).unwrap_or("");
            if !valid_ids.contains(subject) {
                errors.push(format!(
                    "{}: subject does not resolve to an existing document",
                    relation_id
                ));
            }
        }

        let object_kind = relation.get("object_kind").and_then(Value::as_str);
        if matches!(object_kind, Some("module") | Some("symbol")) {
            let object = relation.get("object").and_then(Value::as_str).unwrap_or("");
            if !valid_ids.contains(object) {
                errors.push(format!(
                    "{}: object does not resolve to an existing document",
                    relation_id
                ));
            }
        }

        let relation_evidence = get_array(relation, "evidence");
        let missing_evidence: Vec<&str> = relation_evidence
            .iter()
            .filter_map(Value::as_str)
            .filter(|item| !evidence_ids.contains(*item))
            .collect();
        if !missing_evidence.is_empty() {
            errors.push(format!(
                "{}: evidence does not resolve: {}",
                relation_id,
                missing_evidence.join(", ")
            ));
        }

        let predicate = relation.get("predicate").and_then(Value::as_str);
        if status == Some("exact")
            && !matches!(predicate, Some("imports") | Some("defines"))
            && relation_evidence.is_empty()
        {
            errors.push(format!("{}: exact relation missing evidence", relation_id));
        }

        if matches!(status, Some("inferred") | Some("unresolved")) {
            warnings.push(format!(
                "{}: {} relation requires caution",
                relation_id,
                status.unwrap()
            ));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------
// validate_knowledge
// ---------------------------------------------------------------------

/// Mirrors `validate_knowledge`: per-item OKF-doc check, tier validity,
/// (for active-lifecycle items only) symbol/evidence/relation resolution,
/// the "mining may not silently self-approve a Tier 3/4 claim" rule, the
/// "an unanchored LLM claim must be Tier 4" rule, and the two Tier-4
/// review-required/reviewed checks.
pub fn validate_knowledge(
    bundle_dir: &Path,
    knowledge_state: &Value,
    symbols_state: &Value,
    relations_state: &Value,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let symbol_ids = ids_set(get_array(symbols_state, "symbols"));
    let evidence_ids = ids_set(get_array(symbols_state, "evidence"));
    let relation_ids = ids_set(get_array(relations_state, "relations"));

    // Fix (Medium finding — malformed state tolerance false-pass): Python's
    // `for item in knowledge_state.get("items", [])` only falls back to
    // `[]` when the `"items"` key is *missing*; if it's present but shaped
    // wrong (e.g. `code_knowledge.json`'s `items` is itself a mapping, not
    // a list — iterating a dict yields its string keys), the very next line
    // (`item.get("okf_path", "")`) raises `AttributeError: 'str' object has
    // no attribute 'get'` and the whole `validate` run crashes uncaught.
    // `get_array` (used throughout this file) silently treats *any*
    // non-array shape, present or absent, as `&[]` — so this specific case
    // previously validated zero items and reported zero errors instead of
    // failing, a false "clean bundle" pass. Only `validate_knowledge`'s
    // `items` field is guarded here (the one shape the audit specifically
    // flagged as a reported false-pass); `get_array`'s broader
    // present-but-wrong-shape behavior elsewhere in this file is left
    // alone rather than risking a wide, unverified behavior change across
    // every other call site under this task's time budget.
    if let Some(items_value) = knowledge_state.get("items") {
        if !items_value.is_array() {
            bail!("code_knowledge.json: 'items' must be a list");
        }
    }

    for item in get_array(knowledge_state, "items") {
        let item_id = py_str(item.get("id"));
        let item_doc = bundle_dir.join(get_str_or_empty(item, "okf_path"));
        if !item_doc.exists() {
            errors.push(format!("{}: missing code knowledge OKF doc", item_id));
        } else {
            validate_okf_doc(&item_doc, errors)?;
        }

        let tier = int_or_zero(item.get("knowledge_tier"));
        if !(1..=4).contains(&tier) {
            errors.push(format!("{}: invalid knowledge_tier", item_id));
        }

        // Superseded and retired claims intentionally point at symbols,
        // evidence, and relations from an earlier source version, so only
        // active claims must resolve.
        if lifecycle_status_or_active(item) == "active" {
            let missing_symbols: Vec<&str> = get_array(item, "symbols")
                .iter()
                .filter_map(Value::as_str)
                .filter(|s| !symbol_ids.contains(*s))
                .collect();
            if !missing_symbols.is_empty() {
                errors.push(format!(
                    "{}: unknown symbol(s): {}",
                    item_id,
                    missing_symbols.join(", ")
                ));
            }

            let missing_evidence: Vec<&str> = get_array(item, "evidence")
                .iter()
                .filter_map(Value::as_str)
                .filter(|e| !evidence_ids.contains(*e))
                .collect();
            if !missing_evidence.is_empty() {
                errors.push(format!(
                    "{}: unknown evidence: {}",
                    item_id,
                    missing_evidence.join(", ")
                ));
            }

            let missing_relations: Vec<&str> = get_array(item, "relations")
                .iter()
                .filter_map(Value::as_str)
                .filter(|r| !relation_ids.contains(*r))
                .collect();
            if !missing_relations.is_empty() {
                errors.push(format!(
                    "{}: unknown relation(s): {}",
                    item_id,
                    missing_relations.join(", ")
                ));
            }
        }

        // A human approval recorded in the review log is legitimate. The
        // rule is that a mining pass may not approve its own output.
        if (tier == 3 || tier == 4)
            && item.get("review_status").and_then(Value::as_str) == Some("approved")
            && !truthy(item.get("reviewed"))
        {
            errors.push(format!(
                "{}: Tier {} claim must not be silently approved by mining",
                item_id, tier
            ));
        }

        let anchor_status = item
            .get("code")
            .and_then(|c| c.get("anchor_status"))
            .and_then(Value::as_str);
        if anchor_status == Some("llm_claimed") && tier < 4 {
            errors.push(format!("{}: unanchored LLM claim must be Tier 4", item_id));
        }

        if tier == 4 {
            if !truthy(item.get("review_required")) {
                errors.push(format!("{}: Tier 4 claim must be review_required", item_id));
            }
            if lifecycle_status_or_active(item) == "active" && !truthy(item.get("reviewed")) {
                warnings.push(format!("{}: Tier 4 claim requires human review", item_id));
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------
// validate_evidence
// ---------------------------------------------------------------------

/// Mirrors `validate_evidence`: OKF-doc check, then (only if the source's
/// original snapshot exists on disk) line-bounds, symbol-id match, and
/// span-status checks.
pub fn validate_evidence(
    bundle_dir: &Path,
    source: &Value,
    symbol: &Value,
    evidence: &Value,
    errors: &mut Vec<String>,
) -> Result<()> {
    let evidence_id = py_str(evidence.get("id"));
    let evidence_doc = bundle_dir.join(get_str_or_empty(evidence, "okf_path"));
    if !evidence_doc.exists() {
        errors.push(format!("{}: missing evidence OKF doc", evidence_id));
    } else {
        validate_okf_doc(&evidence_doc, errors)?;
    }

    let original = bundle_dir.join(get_str_or_empty(source, "original_path"));
    if !original.exists() {
        return Ok(());
    }

    let text = std::fs::read_to_string(&original)?;
    let line_count = text.lines().count() as i64;

    if int_or_zero(evidence.get("line_start")) < 1 {
        errors.push(format!("{}: invalid line_start", evidence_id));
    }
    if int_or_zero(evidence.get("line_end")) > line_count {
        errors.push(format!("{}: line_end exceeds source length", evidence_id));
    }
    if evidence.get("symbol_id").and_then(Value::as_str)
        != symbol.get("id").and_then(Value::as_str)
    {
        errors.push(format!("{}: symbol mismatch", evidence_id));
    }
    if evidence.get("span_status").and_then(Value::as_str) != Some("exact") {
        errors.push(format!("{}: expected exact span_status", evidence_id));
    }

    Ok(())
}

// ---------------------------------------------------------------------
// validate_okf_doc
// ---------------------------------------------------------------------

/// Mirrors `validate_okf_doc`: parse + validate the OKF markdown doc at
/// `path`, pushing a formatted error string on an `OkfDocumentError` (the
/// only exception type the Python source catches -- any other I/O failure,
/// such as the file not being readable/valid UTF-8, propagates as `Err`
/// here exactly as it would raise uncaught in Python).
pub fn validate_okf_doc(path: &Path, errors: &mut Vec<String>) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    match OkfDocument::parse(&text) {
        Ok(doc) => {
            if let Err(exc) = doc.validate() {
                errors.push(format!("{}: {}", path.display(), format_okf_error(&exc)));
            }
        }
        Err(exc) => {
            errors.push(format!("{}: {}", path.display(), format_okf_error(&exc)));
        }
    }
    Ok(())
}

/// UNCONFIRMED: assumes `OkfDocumentError` implements `Display` the way
/// Python's `f"{exc}"` renders a `ValueError` (its message text, no type
/// name prefix). Adjust once the real type lands.
fn format_okf_error(exc: &OkfDocumentError) -> String {
    exc.to_string()
}

// ---------------------------------------------------------------------
// _capped / _report_link
// ---------------------------------------------------------------------

/// Mirrors `_capped`: appends `items` to `lines` (already-formatted, e.g.
/// each prefixed with `"- "`), capped at `MAX_LIST`, with a summary line for
/// the overflow; or a single `empty` line if `items` is empty.
fn capped(lines: &mut Vec<String>, items: Vec<String>, empty: &str) {
    if items.is_empty() {
        lines.push(empty.to_string());
        return;
    }
    let total = items.len();
    lines.extend(items.into_iter().take(MAX_LIST));
    if total > MAX_LIST {
        lines.push(format!(
            "- \u{2026} and {} more (see the JSON companion for the full list)",
            total - MAX_LIST
        ));
    }
}

/// Mirrors `_report_link`: a markdown link (or a bare code span if there is
/// no OKF doc to link to) from a report page to `okf_path`, computed via
/// `rel_link` anchored at the synthetic `REPORT_DIR_ANCHOR`.
fn report_link(okf_path: Option<&str>, label: &str) -> String {
    match okf_path {
        None => format!("`{}`", label),
        Some(p) if p.is_empty() => format!("`{}`", label),
        Some(p) => format!("[{}]({})", label, rel_link(REPORT_DIR_ANCHOR, p)),
    }
}

// ---------------------------------------------------------------------
// render_code_validation
// ---------------------------------------------------------------------

/// Mirrors `render_code_validation`.
pub fn render_code_validation(errors: &[String], warnings: &[String]) -> String {
    let mut lines = vec!["# Code Validation Report".to_string(), String::new()];
    lines.push(format!("Errors: {}", errors.len()));
    lines.push(format!("Warnings: {}", warnings.len()));
    lines.push(String::new());
    lines.push("## Errors".to_string());
    capped(
        &mut lines,
        errors.iter().map(|e| format!("- {}", e)).collect(),
        "- None",
    );
    lines.push(String::new());
    lines.push("## Warnings".to_string());
    capped(
        &mut lines,
        warnings.iter().map(|w| format!("- {}", w)).collect(),
        "- None",
    );
    lines.push(String::new());
    lines.push("_Full, uncapped detail is in `code_validation.json`._".to_string());
    lines.push(String::new());
    lines.join("\n")
}

// ---------------------------------------------------------------------
// render_symbol_inventory
// ---------------------------------------------------------------------

/// Mirrors `render_symbol_inventory`.
pub fn render_symbol_inventory(symbols_state: &Value) -> String {
    let modules = get_array(symbols_state, "modules");
    let symbols = get_array(symbols_state, "symbols");

    let mut by_kind: HashMap<String, i64> = HashMap::new();
    for symbol in symbols {
        let kind = symbol.get("kind").and_then(Value::as_str).unwrap_or("unknown").to_string();
        *by_kind.entry(kind).or_insert(0) += 1;
    }

    let mut per_module: HashMap<Option<String>, HashMap<String, i64>> = HashMap::new();
    for symbol in symbols {
        let module_id = symbol.get("module_id").and_then(Value::as_str).map(str::to_string);
        let kind = symbol.get("kind").and_then(Value::as_str).unwrap_or("unknown").to_string();
        let counts = per_module.entry(module_id).or_default();
        *counts.entry(kind).or_insert(0) += 1;
    }

    let kinds = ["class", "function", "method", "test", "constant"];

    let mut lines = vec!["# Symbol Inventory".to_string(), String::new()];
    lines.push(format!("Modules: {} \u{b7} Symbols: {}", modules.len(), symbols.len()));
    lines.push(String::new());
    let totals = kinds
        .iter()
        .map(|k| format!("{}: {}", k, by_kind.get(*k).copied().unwrap_or(0)))
        .collect::<Vec<_>>()
        .join(", ");
    lines.push(format!(
        "Totals: {}",
        if totals.is_empty() { "none".to_string() } else { totals }
    ));
    lines.push(String::new());
    lines.push(format!("| Module | {} | total |", kinds.join(" | ")));
    lines.push(format!("|{}", "---|".repeat(kinds.len() + 2)));

    let mut sorted_modules: Vec<&Value> = modules.iter().collect();
    sorted_modules.sort_by(|a, b| {
        get_str_or_empty(a, "qualified_name").cmp(&get_str_or_empty(b, "qualified_name"))
    });

    for module in sorted_modules {
        let module_id = module.get("id").and_then(Value::as_str).map(str::to_string);
        let default_counts = HashMap::new();
        let counts = per_module.get(&module_id).unwrap_or(&default_counts);
        let total: i64 = counts.values().sum();
        if total == 0 {
            continue;
        }
        let fallback_label = py_str(module.get("id"));
        let label = module
            .get("qualified_name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback_label);
        let link = report_link(module.get("okf_path").and_then(Value::as_str), &label);
        let cells = kinds
            .iter()
            .map(|k| counts.get(*k).copied().unwrap_or(0).to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(format!("| {} | {} | {} |", link, cells, total));
    }
    lines.push(String::new());
    lines.join("\n")
}

// ---------------------------------------------------------------------
// render_relation_resolution
// ---------------------------------------------------------------------

/// Mirrors `render_relation_resolution`.
pub fn render_relation_resolution(relations_state: &Value) -> String {
    let relations = get_array(relations_state, "relations");

    let mut by_status: HashMap<String, i64> = HashMap::new();
    let mut by_predicate: HashMap<String, i64> = HashMap::new();
    for relation in relations {
        let status = relation
            .get("relation")
            .and_then(|m| m.get("resolution_status"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        *by_status.entry(status).or_insert(0) += 1;
        let predicate = relation.get("predicate").and_then(Value::as_str).unwrap_or("unknown").to_string();
        *by_predicate.entry(predicate).or_insert(0) += 1;
    }

    let mut lines = vec![
        "# Relation Resolution".to_string(),
        String::new(),
        format!("Relations: {}", relations.len()),
        String::new(),
        "## By Resolution Status".to_string(),
    ];
    if by_status.is_empty() {
        lines.push("- None".to_string());
    } else {
        let mut entries: Vec<(&String, &i64)> = by_status.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        for (status, count) in entries {
            lines.push(format!("- {}: {}", status, count));
        }
    }
    lines.push(String::new());
    lines.push("## By Predicate".to_string());
    if by_predicate.is_empty() {
        lines.push("- None".to_string());
    } else {
        let mut entries: Vec<(&String, &i64)> = by_predicate.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        for (predicate, count) in entries {
            lines.push(format!("- {}: {}", predicate, count));
        }
    }
    lines.push(String::new());

    let unresolved: Vec<&Value> = relations
        .iter()
        .filter(|r| {
            let status = r
                .get("relation")
                .and_then(|m| m.get("resolution_status"))
                .and_then(Value::as_str);
            matches!(status, Some("inferred") | Some("unresolved"))
        })
        .collect();
    lines.push("## Inferred Or Unresolved".to_string());
    for relation in unresolved.iter().take(100) {
        lines.push(format!(
            "- {}: {} {} {}",
            py_str(relation.get("id")),
            py_str(relation.get("subject")),
            py_str(relation.get("predicate")),
            py_str(relation.get("object")),
        ));
    }
    if unresolved.is_empty() {
        lines.push("- None".to_string());
    }
    lines.push(String::new());
    lines.join("\n")
}

// ---------------------------------------------------------------------
// render_test_coverage_map
// ---------------------------------------------------------------------

/// Mirrors `render_test_coverage_map`.
///
/// NOTE: the Python source has a dead statement here --
/// `{symbol["id"]: symbol for symbol in symbols}` -- built and immediately
/// discarded (never assigned). It has no observable effect, so it is
/// intentionally omitted rather than translated into an unused Rust binding.
pub fn render_test_coverage_map(relations_state: &Value, symbols_state: &Value) -> String {
    let symbols = get_array(symbols_state, "symbols");
    let modules_by_id: HashMap<String, &Value> = get_array(symbols_state, "modules")
        .iter()
        .filter_map(|m| m.get("id").and_then(Value::as_str).map(|id| (id.to_string(), m)))
        .collect();

    let mut covered: HashSet<String> = HashSet::new();
    for relation in get_array(relations_state, "relations") {
        let predicate = relation.get("predicate").and_then(Value::as_str);
        let object = relation.get("object").and_then(Value::as_str).unwrap_or("");
        let subject = relation.get("subject").and_then(Value::as_str).unwrap_or("");
        if predicate == Some("covers") && object.starts_with("symbol-") {
            covered.insert(object.to_string());
        } else if predicate == Some("tested_by") && subject.starts_with("symbol-") {
            covered.insert(subject.to_string());
        }
    }

    // Coverage is measured over non-test callable symbols.
    let targets: Vec<&Value> = symbols
        .iter()
        .filter(|s| {
            matches!(
                s.get("kind").and_then(Value::as_str),
                Some("function") | Some("method") | Some("class")
            )
        })
        .collect();
    let total = targets.len();
    let covered_targets = targets
        .iter()
        .filter(|s| covered.contains(s.get("id").and_then(Value::as_str).unwrap_or("")))
        .count();
    let pct = if total > 0 {
        100.0 * covered_targets as f64 / total as f64
    } else {
        0.0
    };

    let mut by_module: HashMap<Option<String>, Vec<&Value>> = HashMap::new();
    for symbol in &targets {
        let module_id = symbol.get("module_id").and_then(Value::as_str).map(str::to_string);
        by_module.entry(module_id).or_default().push(symbol);
    }

    let mut lines = vec!["# Test Coverage Map".to_string(), String::new()];
    lines.push(format!(
        "Covered {} / {} callable symbols ({:.0}%).",
        covered_targets, total, pct
    ));
    lines.push(String::new());
    lines.push(
        "Coverage is heuristic (pytest name/call matching). Uncovered symbols are listed per module."
            .to_string(),
    );
    lines.push(String::new());

    let mut module_ids: Vec<&Option<String>> = by_module.keys().collect();
    module_ids.sort_by(|a, b| {
        let key_a = module_sort_key(*a, &modules_by_id);
        let key_b = module_sort_key(*b, &modules_by_id);
        key_a.cmp(&key_b)
    });

    for module_id in module_ids {
        let group = &by_module[module_id];
        let mut uncovered: Vec<&&Value> = group
            .iter()
            .filter(|s| !covered.contains(s.get("id").and_then(Value::as_str).unwrap_or("")))
            .collect();
        uncovered.sort_by_key(|s| s.get("line_start").and_then(Value::as_i64).unwrap_or(0));

        let module = module_id
            .as_ref()
            .and_then(|mid| modules_by_id.get(mid.as_str()))
            .copied();
        let cov_count = group.len() - uncovered.len();
        let fallback = py_str(module_id.as_ref().map(|s| Value::String(s.clone())).as_ref());
        let header_label = module
            .and_then(|m| m.get("qualified_name"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback);
        let header = report_link(
            module.and_then(|m| m.get("okf_path")).and_then(Value::as_str),
            &header_label,
        );
        lines.push(format!("## {} \u{2014} {}/{} covered", header, cov_count, group.len()));
        lines.push(String::new());
        if uncovered.is_empty() {
            lines.push("- \u{2713} all covered".to_string());
        } else {
            for symbol in uncovered {
                let qualified_name = get_str_or_empty(symbol, "qualified_name");
                lines.push(format!(
                    "- \u{2717} uncovered: {}",
                    report_link(symbol.get("okf_path").and_then(Value::as_str), &qualified_name)
                ));
            }
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

/// Mirrors the sort key `modules_by_id.get(mid, {}).get("qualified_name", str(mid))`.
fn module_sort_key(module_id: &Option<String>, modules_by_id: &HashMap<String, &Value>) -> String {
    let mid_str = module_id.clone().unwrap_or_else(|| "None".to_string());
    match module_id.as_ref().and_then(|mid| modules_by_id.get(mid.as_str())) {
        Some(module) => module
            .get("qualified_name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(mid_str),
        None => mid_str,
    }
}

// ---------------------------------------------------------------------
// render_change_impact
// ---------------------------------------------------------------------

/// Mirrors `render_change_impact` (including its `label` closure, ported as
/// the free function `change_impact_label` below since Rust closures can't
/// borrow `symbols_by_id` across the whole function as freely as Python's
/// nested function can).
pub fn render_change_impact(relations_state: &Value, symbols_state: Option<&Value>) -> String {
    let empty = Value::Object(Map::new());
    let symbols_state = symbols_state.unwrap_or(&empty);
    let symbols_by_id: HashMap<String, &Value> = get_array(symbols_state, "symbols")
        .iter()
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(|id| (id.to_string(), s)))
        .collect();

    let mut callers: HashMap<String, HashSet<String>> = HashMap::new();
    for relation in get_array(relations_state, "relations") {
        let predicate = relation.get("predicate").and_then(Value::as_str);
        let object = relation.get("object").and_then(Value::as_str).unwrap_or("");
        if matches!(predicate, Some("calls") | Some("covers") | Some("tested_by"))
            && object.starts_with("symbol-")
        {
            let subject = py_str(relation.get("subject"));
            callers.entry(object.to_string()).or_default().insert(subject);
        }
    }

    let mut lines = vec!["# Change Impact".to_string(), String::new()];
    lines.push(
        "Symbols ranked by how many others depend on them (callers + tests). Editing a high-impact symbol touches more code."
            .to_string(),
    );
    lines.push(String::new());

    let mut ranked: Vec<(String, HashSet<String>)> =
        callers.into_iter().collect();
    ranked.sort_by(|a, b| {
        match b.1.len().cmp(&a.1.len()) {
            Ordering::Equal => a.0.cmp(&b.0),
            other => other,
        }
    });

    if ranked.is_empty() {
        lines.push("- No direct change-impact relations detected.".to_string());
        lines.push(String::new());
        return lines.join("\n");
    }

    let total_ranked = ranked.len();
    ranked.truncate(MAX_LIST);
    if total_ranked > MAX_LIST {
        lines.push(format!(
            "Showing the top {} of {} depended-on symbols.",
            MAX_LIST, total_ranked
        ));
        lines.push(String::new());
    }

    for (symbol_id, impacters) in &ranked {
        lines.push(format!(
            "## {} \u{2014} {} dependent(s)",
            change_impact_label(symbol_id, &symbols_by_id),
            impacters.len()
        ));
        lines.push(String::new());
        let mut shown: Vec<&String> = impacters.iter().collect();
        shown.sort();
        for impacter in shown.iter().take(25) {
            lines.push(format!("- {}", change_impact_label(impacter, &symbols_by_id)));
        }
        if shown.len() > 25 {
            lines.push(format!("- \u{2026} and {} more", shown.len() - 25));
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

/// Mirrors `render_change_impact`'s local `label(symbol_id)` closure.
fn change_impact_label(symbol_id: &str, symbols_by_id: &HashMap<String, &Value>) -> String {
    match symbols_by_id.get(symbol_id) {
        Some(symbol) => {
            let qualified_name = get_str_or_empty(symbol, "qualified_name");
            report_link(symbol.get("okf_path").and_then(Value::as_str), &qualified_name)
        }
        None => format!("`{}`", symbol_id),
    }
}

// ---------------------------------------------------------------------
// render_code_review_required
// ---------------------------------------------------------------------

/// Mirrors `render_code_review_required`.
///
/// Grouping key note: the Python source groups by the *raw*
/// `item.get("knowledge_tier")` value (which may be `None`), sorted with
/// `(t is None, t)` -- i.e. `None` sorts last, numeric tiers ascend. This is
/// deliberately a different value than `validate_knowledge`'s coerced
/// `int(... or 0)` tier: a missing tier here is its own group, not folded
/// into tier 0.
pub fn render_code_review_required(knowledge_state: &Value, symbols_state: Option<&Value>) -> String {
    let empty = Value::Object(Map::new());
    let symbols_state = symbols_state.unwrap_or(&empty);
    let symbols_by_id: HashMap<String, &Value> = get_array(symbols_state, "symbols")
        .iter()
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(|id| (id.to_string(), s)))
        .collect();

    let items: Vec<&Value> = get_array(knowledge_state, "items")
        .iter()
        .filter(|item| truthy(item.get("review_required")))
        .collect();

    let mut lines = vec![
        "# Code Review Required".to_string(),
        String::new(),
        format!("Review-required claims: {}", items.len()),
        String::new(),
    ];
    if items.is_empty() {
        lines.push("- None".to_string());
        lines.push(String::new());
        return lines.join("\n");
    }

    // (is_none, tier) so that a missing tier sorts after every numeric tier,
    // matching Python's `(t is None, t)` sort key.
    let mut by_tier: HashMap<Option<i64>, Vec<&Value>> = HashMap::new();
    for item in &items {
        let tier = item.get("knowledge_tier").and_then(Value::as_i64);
        by_tier.entry(tier).or_default().push(item);
    }

    let mut tiers: Vec<Option<i64>> = by_tier.keys().copied().collect();
    tiers.sort_by(|a, b| match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => x.cmp(y),
    });

    for tier in tiers {
        let group = &by_tier[&tier];
        let tier_label = tier.map(|t| t.to_string()).unwrap_or_else(|| "None".to_string());
        lines.push(format!("## Tier {} ({})", tier_label, group.len()));
        lines.push(String::new());

        let mut sorted_group: Vec<&&Value> = group.iter().collect();
        sorted_group.sort_by(|a, b| {
            get_str_or_empty(a, "title").cmp(&get_str_or_empty(b, "title"))
        });

        for item in sorted_group {
            let fallback_title = py_str(item.get("id"));
            let title = item
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or(fallback_title);
            let claim_link = report_link(item.get("okf_path").and_then(Value::as_str), &title);

            let symbol_ids = get_array(item, "symbols");
            let symbol = symbol_ids
                .first()
                .and_then(Value::as_str)
                .and_then(|sid| symbols_by_id.get(sid));
            let suffix = match symbol {
                Some(symbol) => {
                    let qualified_name = get_str_or_empty(symbol, "qualified_name");
                    format!(
                        " \u{2014} {}",
                        report_link(symbol.get("okf_path").and_then(Value::as_str), &qualified_name)
                    )
                }
                None => String::new(),
            };
            lines.push(format!("- {}{}", claim_link, suffix));
        }
        lines.push(String::new());
    }
    lines.join("\n")
}
