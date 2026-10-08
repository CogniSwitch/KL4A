//! Rust port of `kl4a/codekb/trace.py` (Python).
//!
//! Ported from evidence retrieved via the `tools-code` MCP server
//! (code_symbols_search / code_symbols_get / code_tests_for_symbol) for every
//! symbol in the source file. No Python source was read directly.
//!
//! ## Cross-module dependencies (NOT ported in this batch)
//!
//! This file calls into two sibling Python modules that have not yet been
//! ported to Rust as of this batch (no other `.rs` files exist under
//! `codekb/src/` besides `lib.rs`). The `use` declarations below assume the
//! following Rust modules/signatures will exist once those batches land;
//! **the exact signatures are UNCONFIRMED** (only the Python signatures were
//! grounded via MCP) and call sites here may need adjusting once the real
//! modules exist:
//!
//! - `crate::state`:
//!   - `STATE_DIR: &str` (grounded: Python `STATE_DIR = '.codekb'`)
//!   - `read_json(path: &Path, default: Value) -> Result<Value>` (grounded
//!     Python signature: `read_json(path: Path, default: Any) -> Any`; the
//!     Rust error-handling shape -- e.g. whether it silently falls back to
//!     `default` on any read/parse failure, matching the Python name's
//!     implication, vs. propagating a hard error -- is UNCONFIRMED without
//!     reading that port's body)
//!   - `write_json(path: &Path, data: &Value) -> Result<()>` (grounded Python
//!     signature: `write_json(path: Path, data: Any) -> None`)
//! - `crate::canonical`:
//!   - `canonicalize_code_bundle(bundle_dir: &Path) -> Result<Value>`
//!     (grounded Python signature: `canonicalize_code_bundle(bundle_dir: Path) -> dict[str, Any]`)
//!   - `emit_canonical_artifacts(bundle_dir: &Path) -> Result<Value>`
//!     (grounded Python signature: `emit_canonical_artifacts(bundle_dir: Path) -> dict[str, Any]`)
//!   - `canonical_name(name: &str) -> String` (grounded Python signature:
//!     `canonical_name(name: str) -> str`)
//!   - `canonical_kind(kind: &str) -> String` (grounded Python signature:
//!     `canonical_kind(kind: str) -> str`)
//!
//! ## Data shapes
//!
//! Per the shared port convention, `dict[str, Any]`-shaped values (trace
//! documents, trace entries, canonical artifacts/symbols) are kept as
//! `serde_json::Value` rather than being promoted to Rust structs: the
//! Python source itself treats these polymorphically (two different trace
//! entry shapes coexist -- see `trace_entry` vs `create_trace_entries` --
//! and nearly every field is accessed defensively via `.get(...) or {}`
//! rather than a fixed, uniformly-required key set), so a hand-designed
//! struct would be inventing a stability guarantee the original code does
//! not actually rely on.
//!
//! One exception: `dict[str, int]` disposition-count maps are a genuinely
//! stable, simple shape, so `trace_disposition_counts` returns a real
//! `BTreeMap<String, i64>` (whose iteration order is always key-sorted,
//! matching Python's `dict(sorted(counts.items()))`).
//!
//! ## A note on "required" vs "defensive" key access
//!
//! The Python source mixes two access styles: direct indexing
//! (`source_artifact["id"]`), which raises `KeyError` if the key is
//! missing, and defensive `.get(key, default)` / `(x.get(key) or {})`
//! chains, which silently degrade. This port preserves that distinction:
//! direct-indexing call sites become `anyhow` errors (`Result`) here so a
//! malformed/missing required field is still surfaced as a hard failure
//! rather than silently papered over, while defensive-access call sites
//! stay infallible and degrade the same way (empty string / empty
//! list / "unknown") that the Python `.get(..., default)` chains do.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::canonical::{
    canonical_kind, canonical_name, canonicalize_code_bundle, emit_canonical_artifacts,
};
use crate::state::{read_json, write_json, STATE_DIR};

// ---------------------------------------------------------------------------
// small private helpers (not part of the Python symbol list; local plumbing)
// ---------------------------------------------------------------------------

fn get_str<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn get_array(v: &Value, key: &str) -> Vec<Value> {
    v.get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Mirrors Python's `str.title()` for the specific space-separated labels
/// this module renders ("workflow", "state contract"). UNCONFIRMED: Python's
/// `.title()` has additional quirks around apostrophes/digits
/// (e.g. `"it's".title() == "It'S"`) that this simplified word-capitalize
/// does not reproduce; the only real call sites are plain alphabetic labels
/// with no apostrophes, so this is faithful for actual usage.
fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// normalize_name  (trace.py lines 242-243)
// ---------------------------------------------------------------------------

/// `def normalize_name(name: str) -> str: return "".join(ch for ch in name.lower() if ch.isalnum())`
///
/// UNCONFIRMED: `char::is_alphanumeric()` / `str::to_lowercase()` are
/// Unicode-aware like Python's `str.lower()` / `str.isalnum()`, but exotic
/// Unicode case-folding edge cases have not been separately verified against
/// CPython behavior; for the ASCII identifier names this is actually used
/// on, the two are equivalent.
pub fn normalize_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

// ---------------------------------------------------------------------------
// target_artifact_index  (trace.py lines 179-199)
// ---------------------------------------------------------------------------

/// `def target_artifact_index(artifacts: list[dict]) -> dict[tuple[str,str], dict]`
///
/// Builds three index keys per target artifact (exact name+kind, last
/// qualified-name segment+kind, and semantic/canonical name+kind); later
/// writes for the same key overwrite earlier ones, matching Python dict
/// assignment semantics (`index[key] = artifact` executed in source order).
pub fn target_artifact_index(artifacts: &[Value]) -> HashMap<(String, String), Value> {
    let mut index: HashMap<(String, String), Value> = HashMap::new();
    for artifact in artifacts {
        let name = get_str(artifact, "name");
        let kind = get_str(artifact, "kind").to_string();

        index.insert((normalize_name(name), kind.clone()), artifact.clone());

        let qualified_name = get_str(artifact, "qualified_name");
        let last_segment = qualified_name.split('.').next_back().unwrap_or("");
        index.insert((normalize_name(last_segment), kind.clone()), artifact.clone());

        let sem_name = canonical_name(name);
        let sem_kind = canonical_kind(&kind);
        index.insert((normalize_name(&sem_name), sem_kind), artifact.clone());
    }
    index
}

// ---------------------------------------------------------------------------
// trace_entry  (trace.py lines 202-239)
// ---------------------------------------------------------------------------

/// `def trace_entry(source_artifact: dict, target_index: dict) -> dict`
///
/// `source_artifact["id"|"artifact_type"|"kind"|"qualified_name"]` and (on a
/// hit) `target["id"|"qualified_name"]` are direct-indexed in the Python
/// source (i.e. required, `KeyError` on absence), so this port surfaces
/// their absence as an `anyhow` error rather than silently defaulting.
pub fn trace_entry(
    source_artifact: &Value,
    target_index: &HashMap<(String, String), Value>,
) -> Result<Value> {
    let name = get_str(source_artifact, "name");
    let kind = get_str(source_artifact, "kind").to_string();
    let key = (normalize_name(name), kind.clone());
    let semantic_key = (normalize_name(&canonical_name(name)), canonical_kind(&kind));

    // UNCONFIRMED: Python's `target_index.get(key) or target_index.get(semantic_key)`
    // would also fall through to `semantic_key` if the exact-key match resolved to a
    // falsy value (e.g. an empty dict `{}`). Target artifact dicts always carry at
    // least name/kind in practice, so plain Option chaining is behaviorally
    // equivalent for all real inputs; this is an unreachable edge case, not a
    // deliberate behavior choice.
    let target = target_index.get(&key).or_else(|| target_index.get(&semantic_key));

    let (disposition, confidence, target_artifacts) = if let Some(target) = target {
        let artifact_id = target
            .get("id")
            .ok_or_else(|| anyhow!("target artifact missing required key 'id'"))?
            .clone();
        let qualified_name = target
            .get("qualified_name")
            .ok_or_else(|| anyhow!("target artifact missing required key 'qualified_name'"))?
            .clone();
        (
            "preserved",
            vec![json!("inferred")],
            vec![json!({"artifact_id": artifact_id, "qualified_name": qualified_name})],
        )
    } else {
        ("missing", vec![json!("unresolved")], Vec::<Value>::new())
    };

    let review_status = if matches!(disposition, "missing" | "uncertain") {
        "pending"
    } else {
        "not_required"
    };

    let source_id = source_artifact
        .get("id")
        .ok_or_else(|| anyhow!("source artifact missing required key 'id'"))?
        .clone();
    let source_artifact_type = source_artifact
        .get("artifact_type")
        .ok_or_else(|| anyhow!("source artifact missing required key 'artifact_type'"))?
        .clone();
    let source_kind = source_artifact
        .get("kind")
        .ok_or_else(|| anyhow!("source artifact missing required key 'kind'"))?
        .clone();
    let source_qualified_name = source_artifact
        .get("qualified_name")
        .ok_or_else(|| anyhow!("source artifact missing required key 'qualified_name'"))?
        .clone();
    let source_evidence = source_artifact
        .get("evidence")
        .cloned()
        .unwrap_or_else(|| json!([]));

    Ok(json!({
        "source": {
            "artifact_id": source_id,
            "artifact_type": source_artifact_type,
            "kind": source_kind,
            "qualified_name": source_qualified_name,
            "evidence": source_evidence,
        },
        "target": {"artifacts": target_artifacts},
        "mapping": {"disposition": disposition, "confidence": confidence},
        "review": {"status": review_status},
    }))
}

// ---------------------------------------------------------------------------
// trace_summary  (trace.py lines 246-251)
// ---------------------------------------------------------------------------

/// `def trace_summary(entries: list[dict]) -> dict`
///
/// `entry["mapping"]["disposition"]` is direct-indexed in Python (required).
pub fn trace_summary(entries: &[Value]) -> Result<Value> {
    let mut by_disposition: BTreeMap<String, i64> = BTreeMap::new();
    for entry in entries {
        let disposition = entry
            .get("mapping")
            .and_then(|m| m.get("disposition"))
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("trace entry missing required key 'mapping.disposition'"))?;
        *by_disposition.entry(disposition.to_string()).or_insert(0) += 1;
    }
    Ok(json!({"entry_count": entries.len(), "by_disposition": by_disposition}))
}

// ---------------------------------------------------------------------------
// render_trace_coverage  (trace.py lines 254-267)
// ---------------------------------------------------------------------------

/// `def render_trace_coverage(trace: dict) -> str`
///
/// `trace['source_bundle']`, `trace['target_bundle']`,
/// `trace['summary']['entry_count']`, `trace['summary']['by_disposition']`
/// are all direct-indexed in Python (required).
pub fn render_trace_coverage(trace: &Value) -> Result<String> {
    let mut lines = vec!["# Cross-KB Trace Coverage".to_string(), String::new()];

    let source_bundle = trace
        .get("source_bundle")
        .ok_or_else(|| anyhow!("trace missing required key 'source_bundle'"))?;
    let target_bundle = trace
        .get("target_bundle")
        .ok_or_else(|| anyhow!("trace missing required key 'target_bundle'"))?;
    lines.push(format!("Source bundle: `{}`", display(source_bundle)));
    lines.push(format!("Target bundle: `{}`", display(target_bundle)));
    lines.push(String::new());

    let summary = trace
        .get("summary")
        .ok_or_else(|| anyhow!("trace missing required key 'summary'"))?;
    let entry_count = summary
        .get("entry_count")
        .ok_or_else(|| anyhow!("summary missing required key 'entry_count'"))?;
    lines.push(format!("Entries: {}", entry_count));
    lines.push(String::new());
    lines.push("## By Disposition".to_string());

    let by_disposition = summary
        .get("by_disposition")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("summary missing required key 'by_disposition'"))?;
    let mut items: Vec<(&String, &Value)> = by_disposition.iter().collect();
    items.sort_by(|a, b| a.0.cmp(b.0));
    for (disposition, count) in &items {
        lines.push(format!("- {}: {}", disposition, count));
    }
    if items.is_empty() {
        lines.push("- None".to_string());
    }
    lines.push(String::new());
    Ok(lines.join("\n"))
}

/// Renders a JSON string value the way an f-string would render a Python
/// str (no surrounding quotes); falls back to serde_json's Display for
/// non-string values.
fn display(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// render_trace_gaps  (trace.py lines 270-294)
// ---------------------------------------------------------------------------

/// `def render_trace_gaps(trace: dict) -> str`
///
/// All field access in the Python source is defensive (`.get(...) or {}` /
/// `... or "unknown"`), so this is infallible.
pub fn render_trace_gaps(trace: &Value) -> String {
    let entries = get_array(trace, "entries");
    let missing: Vec<&Value> = entries
        .iter()
        .filter(|entry| {
            let disposition = entry
                .get("mapping")
                .and_then(|m| m.get("disposition"))
                .and_then(Value::as_str)
                .unwrap_or("");
            matches!(disposition, "missing" | "uncertain" | "deferred")
        })
        .collect();

    let mut lines = vec![
        "# Cross-KB Trace Gaps".to_string(),
        String::new(),
        format!("Open gaps: {}", missing.len()),
        String::new(),
    ];
    if missing.is_empty() {
        lines.push("- None".to_string());
        lines.push(String::new());
        return lines.join("\n");
    }

    let mut by_kind: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for entry in &missing {
        let kind = entry
            .get("source")
            .and_then(|s| s.get("kind"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("unknown")
            .to_string();
        by_kind.entry(kind).or_default().push(entry);
    }
    for (kind, kind_entries) in &by_kind {
        lines.push(format!("## {}", kind));
        for entry in kind_entries {
            let source = entry.get("source");
            let qualified_name = source
                .and_then(|s| s.get("qualified_name"))
                .map(display)
                .unwrap_or_default();
            let artifact_id = source
                .and_then(|s| s.get("artifact_id"))
                .map(display)
                .unwrap_or_default();
            lines.push(format!("- `{}` ({})", qualified_name, artifact_id));
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// render_kind_equivalence  (trace.py lines 297-331)
// ---------------------------------------------------------------------------

/// `def render_kind_equivalence(trace: dict, label: str, kinds: set[str]) -> str`
///
/// All field access in the Python source is defensive, so this is infallible.
pub fn render_kind_equivalence(trace: &Value, label: &str, kinds: &HashSet<String>) -> String {
    let entries = get_array(trace, "entries");
    let relevant: Vec<&Value> = entries
        .iter()
        .filter(|entry| {
            let kind = entry
                .get("source")
                .and_then(|s| s.get("kind"))
                .and_then(Value::as_str)
                .unwrap_or("");
            kinds.contains(kind)
        })
        .collect();
    let preserved_count = relevant
        .iter()
        .filter(|entry| {
            entry
                .get("mapping")
                .and_then(|m| m.get("disposition"))
                .and_then(Value::as_str)
                == Some("preserved")
        })
        .count();

    let mut lines = vec![
        format!("# {} Equivalence", title_case(label)),
        String::new(),
        format!("Source {} artifacts: {}", label, relevant.len()),
        format!("Preserved mappings: {}", preserved_count),
        String::new(),
    ];
    if relevant.is_empty() {
        lines.push("- None".to_string());
        lines.push(String::new());
        return lines.join("\n");
    }
    for entry in &relevant {
        let source = entry.get("source");
        let qualified_name = source
            .and_then(|s| s.get("qualified_name"))
            .map(display)
            .unwrap_or_default();
        let target_artifacts = entry
            .get("target")
            .and_then(|t| t.get("artifacts"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let disposition = entry
            .get("mapping")
            .and_then(|m| m.get("disposition"))
            .map(display)
            .unwrap_or_default();
        let target_str = if target_artifacts.is_empty() {
            "unmapped".to_string()
        } else {
            target_artifacts
                .iter()
                .map(|a| {
                    format!(
                        "`{}`",
                        a.get("qualified_name").map(display).unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        lines.push(format!(
            "- `{}` -> {} ({})",
            qualified_name, target_str, disposition
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// render_rule_equivalence  (trace.py lines 334-358)
// ---------------------------------------------------------------------------

/// `def render_rule_equivalence(source: dict, target: dict) -> str`
///
/// All field access in the Python source is defensive, so this is infallible.
pub fn render_rule_equivalence(source: &Value, target: &Value) -> String {
    let source_rules = get_array(source, "rules");
    let target_rules = get_array(target, "rules");
    let mut lines = vec![
        "# Rule Equivalence".to_string(),
        String::new(),
        format!("Source proposed rules: {}", source_rules.len()),
        format!("Target proposed rules: {}", target_rules.len()),
        String::new(),
        "Rule equivalence is reported as review-needed because generated target code preserves traceable structure before human-approved behavior parity.".to_string(),
        String::new(),
    ];
    if source_rules.is_empty() {
        lines.push("- No source rules found".to_string());
        lines.push(String::new());
        return lines.join("\n");
    }
    for rule in &source_rules {
        let review_status = rule.get("review_status").and_then(Value::as_str);
        let status = if review_status != Some("approved") {
            "review-needed"
        } else {
            "approved-source"
        };
        let id = rule.get("id").map(display).unwrap_or_default();
        lines.push(format!("- `{}`: {}", id, status));
    }
    lines.push(String::new());
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// trace_disposition_counts  (trace.py lines 361-368)
// ---------------------------------------------------------------------------

/// `def trace_disposition_counts(entries: list[dict]) -> dict[str, int]`
///
/// Tries a flat `entry["disposition"]` field first (the shape produced by
/// `create_trace_entries`), then falls back to `entry["mapping"]["disposition"]`
/// (the shape produced by `trace_entry`), then `"unknown"` -- matching
/// Python's `entry.get("disposition") or (entry.get("mapping") or {}).get("disposition", "unknown")`.
/// A `BTreeMap` is used so iteration/serialization is always key-sorted,
/// matching Python's `dict(sorted(counts.items()))`.
pub fn trace_disposition_counts(entries: &[Value]) -> BTreeMap<String, i64> {
    let mut counts: BTreeMap<String, i64> = BTreeMap::new();
    for entry in entries {
        let disposition = entry
            .get("disposition")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                entry
                    .get("mapping")
                    .and_then(Value::as_object)
                    .and_then(|m| m.get("disposition"))
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or("unknown");
        *counts.entry(disposition.to_string()).or_insert(0) += 1;
    }
    counts
}

// ---------------------------------------------------------------------------
// load_or_create_canonical  (trace.py lines 172-176)
// ---------------------------------------------------------------------------

/// `def load_or_create_canonical(bundle_dir: Path) -> dict`
pub fn load_or_create_canonical(bundle_dir: &Path) -> Result<Value> {
    let path = bundle_dir.join(STATE_DIR).join("code_canonical.json");
    if path.exists() {
        return Ok(read_json(&path, json!({})));
    }
    canonicalize_code_bundle(bundle_dir)
}

// ---------------------------------------------------------------------------
// create_trace_entries  (trace.py lines 119-169)
// ---------------------------------------------------------------------------

/// `def create_trace_entries(bundle_dir, *, source_languages, target_languages, canonical_state=None) -> dict`
///
/// Note this produces a *different* trace-entry shape than `trace_entry`
/// does (flat `"disposition"`/`"targets"` keys, keyed by pre-existing
/// `"canonical_name"`/`"canonical_kind"` fields on each canonical symbol --
/// NOT the `canonical_name()`/`canonical_kind()` functions from
/// `crate::canonical`, which are a different, artifact-dict-oriented
/// normalization used by `target_artifact_index`/`trace_entry`). This writes
/// `<bundle_dir>/<STATE_DIR>/code_trace.json` and is consumed by
/// `trace_validate::validate_trace_coverage`, not by the `trace.json`
/// pipeline (`create_cross_kb_trace`/`review_trace_entry`/etc).
pub fn create_trace_entries(
    bundle_dir: &Path,
    source_languages: &HashSet<String>,
    target_languages: &HashSet<String>,
    canonical_state: Option<Value>,
) -> Result<Value> {
    let canonical = match canonical_state {
        Some(c) => c,
        None => emit_canonical_artifacts(bundle_dir)?,
    };
    let symbols = get_array(&canonical, "symbols");

    let source_symbols: Vec<Value> = symbols
        .iter()
        .filter(|item| {
            item.get("language")
                .and_then(Value::as_str)
                .map(|lang| source_languages.contains(lang))
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    let target_symbols: Vec<Value> = symbols
        .iter()
        .filter(|item| {
            item.get("language")
                .and_then(Value::as_str)
                .map(|lang| target_languages.contains(lang))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    let mut target_index: HashMap<(String, String), Value> = HashMap::new();
    for symbol in &target_symbols {
        let key = (
            get_str(symbol, "canonical_name").to_string(),
            get_str(symbol, "canonical_kind").to_string(),
        );
        target_index.insert(key, symbol.clone());
    }

    let mut entries: Vec<Value> = Vec::new();
    for source in &source_symbols {
        let key = (
            get_str(source, "canonical_name").to_string(),
            get_str(source, "canonical_kind").to_string(),
        );
        if let Some(target) = target_index.get(&key) {
            entries.push(json!({
                "source": source,
                "targets": [target],
                "disposition": "mapped",
                "heuristic": "exact-normalized-name-kind",
            }));
        } else {
            entries.push(json!({
                "source": source,
                "targets": Vec::<Value>::new(),
                "disposition": "unresolved",
                "heuristic": "no-normalized-name-kind-match",
            }));
        }
    }

    let by_disposition = trace_disposition_counts(&entries);
    let trace = json!({
        "entries": entries,
        "summary": {"by_disposition": by_disposition},
    });

    let path = bundle_dir.join(STATE_DIR).join("code_trace.json");
    write_json(&path, &trace)?;
    Ok(trace)
}

// ---------------------------------------------------------------------------
// create_cross_kb_trace  (trace.py lines 15-54)
// ---------------------------------------------------------------------------

/// `def create_cross_kb_trace(source_bundle_dir, target_bundle_dir, trace_dir) -> dict`
pub fn create_cross_kb_trace(
    source_bundle_dir: &Path,
    target_bundle_dir: &Path,
    trace_dir: &Path,
) -> Result<Value> {
    let source = load_or_create_canonical(source_bundle_dir)?;
    let target = load_or_create_canonical(target_bundle_dir)?;
    let target_artifacts = get_array(&target, "artifacts");
    let target_index = target_artifact_index(&target_artifacts);

    let source_artifacts = get_array(&source, "artifacts");
    let mut entries = Vec::with_capacity(source_artifacts.len());
    for source_artifact in &source_artifacts {
        entries.push(trace_entry(source_artifact, &target_index)?);
    }

    // Python: `source_bundle_dir.name` / `target_bundle_dir.name` (final path component).
    let source_bundle_name = source_bundle_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let target_bundle_name = target_bundle_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let summary = trace_summary(&entries)?;
    let trace = json!({
        "source_bundle": source_bundle_name,
        "target_bundle": target_bundle_name,
        "entries": entries,
        "summary": summary,
    });

    fs::create_dir_all(trace_dir)
        .with_context(|| format!("creating trace dir {}", trace_dir.display()))?;
    write_json(&trace_dir.join("trace.json"), &trace)?;

    let reports_dir = trace_dir.join("reports");
    fs::create_dir_all(&reports_dir)
        .with_context(|| format!("creating reports dir {}", reports_dir.display()))?;

    fs::write(reports_dir.join("coverage.md"), render_trace_coverage(&trace)?)?;
    fs::write(reports_dir.join("gaps.md"), render_trace_gaps(&trace))?;

    let workflow_kinds: HashSet<String> = ["program", "paragraph", "function", "method"]
        .into_iter()
        .map(String::from)
        .collect();
    fs::write(
        reports_dir.join("workflow_equivalence.md"),
        render_kind_equivalence(&trace, "workflow", &workflow_kinds),
    )?;

    let state_contract_kinds: HashSet<String> = ["record", "data_item", "file", "class"]
        .into_iter()
        .map(String::from)
        .collect();
    fs::write(
        reports_dir.join("state_contract_equivalence.md"),
        render_kind_equivalence(&trace, "state contract", &state_contract_kinds),
    )?;

    fs::write(
        reports_dir.join("rule_equivalence.md"),
        render_rule_equivalence(&source, &target),
    )?;

    Ok(trace)
}

// ---------------------------------------------------------------------------
// review_trace_entry  (trace.py lines 57-116)
// ---------------------------------------------------------------------------

/// `def review_trace_entry(trace_dir, *, source_artifact_id, disposition, reviewer, rationale, target_artifacts=None) -> dict`
///
/// Every disposition branch from the Python `valid_dispositions` set is
/// handled generically here (not enumerated case-by-case): `renamed`,
/// `split`, `merged`, `replaced`, `retired` all set `review.status =
/// "approved"`; `deferred` and `uncertain` set `review.status = "pending"`.
/// This matches the Python `disposition in {"renamed","split","merged","replaced","retired"}`
/// membership test exactly (grounded further by
/// `test_trace_review_supports_renamed_split_and_retired_dispositions`,
/// which exercises the `retired` and `split` branches end-to-end via the
/// CLI and asserts `review.status == "approved"` for both).
pub fn review_trace_entry(
    trace_dir: &Path,
    source_artifact_id: &str,
    disposition: &str,
    reviewer: &str,
    rationale: &str,
    target_artifacts: Option<Vec<String>>,
) -> Result<Value> {
    let trace_path = trace_dir.join("trace.json");
    let mut trace = read_json(&trace_path, json!({"entries": []}));

    const VALID_DISPOSITIONS: [&str; 7] = [
        "renamed", "split", "merged", "replaced", "retired", "deferred", "uncertain",
    ];
    if !VALID_DISPOSITIONS.contains(&disposition) {
        let mut sorted = VALID_DISPOSITIONS.to_vec();
        sorted.sort_unstable();
        bail!("review disposition must be one of: {}", sorted.join(", "));
    }

    let entries = trace
        .get_mut("entries")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("trace missing required key 'entries'"))?;

    let idx = entries
        .iter()
        .position(|item| {
            item.get("source")
                .and_then(|s| s.get("artifact_id"))
                .and_then(Value::as_str)
                == Some(source_artifact_id)
        })
        .ok_or_else(|| {
            anyhow!(
                "trace entry not found for source artifact: {}",
                source_artifact_id
            )
        })?;

    {
        let entry = &mut entries[idx];
        let mapping = entry
            .get_mut("mapping")
            .ok_or_else(|| anyhow!("trace entry missing required key 'mapping'"))?;
        mapping["disposition"] = json!(disposition);
        mapping["confidence"] = json!(["reviewed"]);

        if let Some(targets) = &target_artifacts {
            let target_artifacts_json: Vec<Value> = targets
                .iter()
                .map(|t| json!({"artifact_id": t, "qualified_name": t}))
                .collect();
            entry["target"]["artifacts"] = json!(target_artifacts_json);
        }

        const APPROVED_DISPOSITIONS: [&str; 5] =
            ["renamed", "split", "merged", "replaced", "retired"];
        let status = if APPROVED_DISPOSITIONS.contains(&disposition) {
            "approved"
        } else {
            "pending"
        };
        entry["review"] = json!({"status": status, "reviewer": reviewer, "rationale": rationale});
    }

    let entries_snapshot: Vec<Value> = entries.clone();
    let summary = trace_summary(&entries_snapshot)?;
    trace["summary"] = summary;

    write_json(&trace_path, &trace)?;

    let reports_dir = trace_dir.join("reports");
    fs::create_dir_all(&reports_dir)
        .with_context(|| format!("creating reports dir {}", reports_dir.display()))?;
    fs::write(reports_dir.join("coverage.md"), render_trace_coverage(&trace)?)?;
    fs::write(reports_dir.join("gaps.md"), render_trace_gaps(&trace))?;

    Ok(trace["entries"][idx].clone())
}
