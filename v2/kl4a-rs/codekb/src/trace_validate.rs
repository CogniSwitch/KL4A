//! Rust port of `kl4a/codekb/trace_validate.py` (Python).
//!
//! Ported from evidence retrieved via the `tools-code` MCP server
//! (code_symbols_search / code_symbols_get / code_tests_for_symbol) for every
//! symbol in the source file. No Python source was read directly.
//!
//! ## Cross-module dependencies (NOT ported in this batch)
//!
//! - `crate::state::read_json(path: &Path, default: Value) -> Result<Value>`
//!   and `crate::state::write_json(path: &Path, data: &Value) -> Result<()>`
//!   -- see the equivalent note at the top of `trace.rs`; the exact Rust
//!   error-handling shape is UNCONFIRMED until that port batch lands.
//!
//! ## Data shapes
//!
//! `dict[str, Any]`-shaped trace/canonical documents are kept as
//! `serde_json::Value`, matching `trace.rs` and for the same reason: the
//! Python source accesses nearly every field defensively
//! (`entry.get(...) or {}`), which is not the access pattern of a stable,
//! uniformly-required schema.
//!
//! ## A note on well-formed-input assumptions
//!
//! `validate_trace_coverage`'s Python source builds
//! `by_disposition[disposition] = ... ` where `disposition = entry.get("disposition")`
//! (which is `None`, the Python object, if the key is absent), and later does
//! `dict(sorted(by_disposition.items()))` -- sorting a dict whose keys mix
//! `None` and `str` raises `TypeError` in CPython 3. So the Python original
//! already assumes every trace entry has a `"disposition"` string (true for
//! every entry `create_trace_entries` produces) and would crash, not
//! gracefully degrade, if fed a malformed entry lacking one. This port does
//! not reproduce that crash for malformed input -- missing/non-string
//! `"disposition"`/`"id"`/etc. fields degrade to `""`/`"unknown"` here
//! instead of panicking -- since no test evidence exercises that failure
//! path and silently mismatching Python's crash is preferable to inventing
//! a specific panic message. This is a deliberate, documented divergence
//! for malformed/adversarial input only; well-formed pipeline data (the
//! only case the original code is actually exercised with) behaves
//! identically.

use anyhow::Result;
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use crate::state::{read_json, write_json};

/// `CRITICAL_KINDS = {'program', 'paragraph', 'function', 'method', 'record', 'file'}`
/// (trace_validate.py line 8)
pub static CRITICAL_KINDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "program",
        "paragraph",
        "function",
        "method",
        "record",
        "file",
    ]
    .into_iter()
    .collect()
});

// ---------------------------------------------------------------------------
// validate_cross_kb_trace  (trace_validate.py lines 11-61)
// ---------------------------------------------------------------------------

/// `def validate_cross_kb_trace(trace_dir: Path) -> tuple[list[str], list[str]]`
///
/// Every individual check inside this validator is its own rule, ported in
/// full (none dropped):
/// 1. disposition must be one of the 9 known values (else: error)
/// 2. a `CRITICAL_KINDS` source artifact with disposition `"missing"` (warning)
/// 3. a `"retired"` disposition whose review status isn't `"approved"` (error)
/// 4. a `"renamed"/"split"/"merged"/"replaced"` disposition whose review
///    status isn't `"approved"` (error)
/// 5. a `"renamed"/"split"/"merged"/"replaced"` disposition with no target
///    artifact(s) (error)
pub fn validate_cross_kb_trace(trace_dir: &Path) -> Result<(Vec<String>, Vec<String>)> {
    let trace = read_json(&trace_dir.join("trace.json"), json!({"entries": []}));
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let valid_dispositions: HashSet<&str> = [
        "preserved", "renamed", "split", "merged", "replaced", "retired", "deferred", "missing",
        "uncertain",
    ]
    .into_iter()
    .collect();

    let entries = trace
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for entry in &entries {
        let source = entry.get("source");
        let mapping = entry.get("mapping");
        let disposition = mapping
            .and_then(|m| m.get("disposition"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let artifact_id = source
            .and_then(|s| s.get("artifact_id"))
            .and_then(Value::as_str)
            .unwrap_or("");

        // Rule 1: disposition must be a known value.
        if !valid_dispositions.contains(disposition) {
            errors.push(format!("{}: invalid disposition {}", artifact_id, disposition));
        }

        // Rule 2: a critical-kind artifact must not be missing.
        let kind = source
            .and_then(|s| s.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if CRITICAL_KINDS.contains(kind) && disposition == "missing" {
            warnings.push(format!("{}: critical artifact is missing", artifact_id));
        }

        let review_status = entry
            .get("review")
            .and_then(|r| r.get("status"))
            .and_then(Value::as_str);

        // Rule 3: a retired artifact requires an approved review.
        if disposition == "retired" && review_status != Some("approved") {
            errors.push(format!(
                "{}: retired artifact requires approved review",
                artifact_id
            ));
        }

        // Rules 4 & 5: renamed/split/merged/replaced require an approved
        // review AND at least one target artifact.
        if matches!(disposition, "renamed" | "split" | "merged" | "replaced") {
            if review_status != Some("approved") {
                errors.push(format!(
                    "{}: {} mapping requires approved review",
                    artifact_id, disposition
                ));
            }
            let has_targets = entry
                .get("target")
                .and_then(|t| t.get("artifacts"))
                .and_then(Value::as_array)
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            if !has_targets {
                errors.push(format!(
                    "{}: {} mapping requires target artifact(s)",
                    artifact_id, disposition
                ));
            }
        }
    }

    let reports_dir = trace_dir.join("reports");
    fs::create_dir_all(&reports_dir)?;
    write_json(
        &reports_dir.join("trace_validation.json"),
        &json!({"errors": errors, "warnings": warnings}),
    )?;
    fs::write(
        reports_dir.join("validation.md"),
        render_trace_validation(&errors, &warnings),
    )?;

    Ok((errors, warnings))
}

// ---------------------------------------------------------------------------
// validate_trace_coverage  (trace_validate.py lines 64-117)
// ---------------------------------------------------------------------------

/// `def validate_trace_coverage(bundle_dir, *, source_languages, target_languages, canonical_state, trace_state) -> dict`
///
/// This validates the *other* trace shape -- the flat `code_trace.json`
/// produced by `trace::create_trace_entries` (`entry["disposition"]`,
/// `entry["source"]["id"]`, `entry["targets"]`), not the `trace.json` shape
/// `validate_cross_kb_trace` checks. Individual rules ported:
/// 1. disposition must be one of `{"mapped","candidate","unresolved","excluded"}` (error)
/// 2. an `"unresolved"` disposition needs review (warning)
/// 3. every expected source symbol id must have a trace entry (error, "missing trace disposition")
/// plus a coverage `summary` (sources/targets expected vs. covered) written
/// alongside the errors/warnings.
pub fn validate_trace_coverage(
    bundle_dir: &Path,
    source_languages: &HashSet<String>,
    target_languages: &HashSet<String>,
    canonical_state: &Value,
    trace_state: &Value,
) -> Result<Value> {
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let valid_dispositions: HashSet<&str> = ["mapped", "candidate", "unresolved", "excluded"]
        .into_iter()
        .collect();

    let symbols = canonical_state
        .get("symbols")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let source_ids: HashSet<String> = symbols
        .iter()
        .filter(|s| {
            s.get("language")
                .and_then(Value::as_str)
                .map(|l| source_languages.contains(l))
                .unwrap_or(false)
        })
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(str::to_string))
        .collect();
    let target_ids: HashSet<String> = symbols
        .iter()
        .filter(|s| {
            s.get("language")
                .and_then(Value::as_str)
                .map(|l| target_languages.contains(l))
                .unwrap_or(false)
        })
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(str::to_string))
        .collect();

    let mut seen_source_ids: HashSet<String> = HashSet::new();
    let mut inbound_target_ids: HashSet<String> = HashSet::new();
    let mut by_disposition: BTreeMap<String, i64> = BTreeMap::new();

    let entries = trace_state
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for entry in &entries {
        // See the module-level note: Python assumes "disposition" is always
        // present (a str). Absent here degrades to "" instead of Python's
        // `None`-sorts-crash.
        let disposition = entry
            .get("disposition")
            .and_then(Value::as_str)
            .unwrap_or("");
        let source_id = entry
            .get("source")
            .and_then(|s| s.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("");

        *by_disposition.entry(disposition.to_string()).or_insert(0) += 1;

        if !valid_dispositions.contains(disposition) {
            errors.push(format!("{}: invalid disposition {}", source_id, disposition));
        }
        if !source_id.is_empty() {
            seen_source_ids.insert(source_id.to_string());
        }
        if disposition == "unresolved" {
            warnings.push(format!("{}: unresolved trace requires review", source_id));
        }
        if let Some(targets) = entry.get("targets").and_then(Value::as_array) {
            for target in targets {
                if let Some(tid) = target.get("id").and_then(Value::as_str) {
                    if target_ids.contains(tid) {
                        inbound_target_ids.insert(tid.to_string());
                    }
                }
            }
        }
    }

    let mut missing_sources: Vec<&String> = source_ids.difference(&seen_source_ids).collect();
    missing_sources.sort();
    for source_id in missing_sources {
        errors.push(format!("{}: missing trace disposition", source_id));
    }

    let reports_dir = bundle_dir.join("reports");
    fs::create_dir_all(&reports_dir)?;

    let summary = json!({
        "sources_expected": source_ids.len(),
        "sources_with_disposition": source_ids.intersection(&seen_source_ids).count(),
        "targets_expected": target_ids.len(),
        "targets_with_inbound_trace": inbound_target_ids.len(),
        "by_disposition": by_disposition,
    });
    let result = json!({"errors": errors, "warnings": warnings, "summary": summary});

    write_json(&reports_dir.join("code_trace_validation.json"), &result)?;
    fs::write(
        reports_dir.join("code_trace_validation.md"),
        render_coverage_validation(&result)?,
    )?;

    Ok(result)
}

// ---------------------------------------------------------------------------
// render_coverage_validation  (trace_validate.py lines 141-161)
// ---------------------------------------------------------------------------

/// `def render_coverage_validation(result: dict) -> str`
///
/// `result['errors']`, `result['warnings']`, `result["summary"]`,
/// `result["summary"]["by_disposition"]` are all direct-indexed in Python
/// (required).
///
/// The `## Coverage` section iterates `result["summary"].items()` in
/// Python's dict insertion order (`sources_expected`,
/// `sources_with_disposition`, `targets_expected`,
/// `targets_with_inbound_trace`, per `validate_trace_coverage`'s
/// construction order). `serde_json::Value`'s object map does not guarantee
/// insertion-order iteration unless the crate's `preserve_order` feature is
/// enabled (UNCONFIRMED whether it is, for this workspace), so this hardcodes
/// that known key order for robustness rather than relying on map iteration
/// order.
pub fn render_coverage_validation(result: &Value) -> Result<String> {
    let errors = result
        .get("errors")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("result missing required key 'errors'"))?;
    let warnings = result
        .get("warnings")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("result missing required key 'warnings'"))?;
    let mut lines = vec![
        "# Code Trace Validation".to_string(),
        String::new(),
        format!("Errors: {}", errors.len()),
        format!("Warnings: {}", warnings.len()),
        String::new(),
        "## Coverage".to_string(),
    ];

    let summary = result
        .get("summary")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("result missing required key 'summary'"))?;

    const KNOWN_ORDER: [&str; 4] = [
        "sources_expected",
        "sources_with_disposition",
        "targets_expected",
        "targets_with_inbound_trace",
    ];
    for key in KNOWN_ORDER {
        if let Some(value) = summary.get(key) {
            lines.push(format!("- {}: {}", key, value));
        }
    }
    // Forward-compat fallback for any summary key not in the known order
    // (and not "by_disposition", which gets its own section below).
    for (key, value) in summary {
        if key == "by_disposition" || KNOWN_ORDER.contains(&key.as_str()) {
            continue;
        }
        lines.push(format!("- {}: {}", key, value));
    }

    lines.push(String::new());
    lines.push("## By Disposition".to_string());
    let by_disposition = summary
        .get("by_disposition")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("summary missing required key 'by_disposition'"))?;
    for (disposition, count) in by_disposition {
        lines.push(format!("- {}: {}", disposition, count));
    }
    if by_disposition.is_empty() {
        lines.push("- None".to_string());
    }
    lines.push(String::new());
    Ok(lines.join("\n"))
}

// ---------------------------------------------------------------------------
// render_trace_validation  (trace_validate.py lines 120-138)
// ---------------------------------------------------------------------------

/// `def render_trace_validation(errors: list[str], warnings: list[str]) -> str`
///
/// Plain list parameters, no dict access at all -- infallible.
pub fn render_trace_validation(errors: &[String], warnings: &[String]) -> String {
    let mut lines = vec![
        "# Cross-KB Trace Validation".to_string(),
        String::new(),
        format!("Errors: {}", errors.len()),
        format!("Warnings: {}", warnings.len()),
        String::new(),
        "## Errors".to_string(),
    ];
    for error in errors {
        lines.push(format!("- {}", error));
    }
    if errors.is_empty() {
        lines.push("- None".to_string());
    }
    lines.push(String::new());
    lines.push("## Warnings".to_string());
    for warning in warnings {
        lines.push(format!("- {}", warning));
    }
    if warnings.is_empty() {
        lines.push("- None".to_string());
    }
    lines.push(String::new());
    lines.join("\n")
}
