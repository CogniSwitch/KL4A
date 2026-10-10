//! Port of `kl4a/codekb/agent.py`.
//!
//! The whole Python file is a single function, `run_code_agent_harness`: a
//! fixture-provider "code agent" harness that answers a task/query pair by
//! retrieving grounded context through the Code Knowledge Access Layer
//! (`code_context`) and summarizing what it found — it never touches the UI
//! and never fabricates an answer beyond what `code_context` returned.
//!
//! Ported symbol-for-symbol from the Python source (verified via tools-code
//! MCP `code_symbols_get`/`code_context`). There is no dispatch/branching to
//! enumerate: the only conditional in the Python is the `provider != "fixture"`
//! guard, ported below as-is.
//!
//! ## Cross-batch dependency (NOT part of this batch — report only)
//!
//! `kl4a.codekb.context.code_context` (source file `kl4a/codekb/context.py`,
//! not in this batch) is expected here as `crate::context::code_context`,
//! taking `(bundle_dir, task, query, language)` and returning a JSON object
//! shaped like the Python dict: `{"symbols": [...], "relations": [...],
//! "evidence": [...], "code_knowledge": [...], "tests": [...], "warnings":
//! [...], "context_rules": [...], "task": {...}}` (field names confirmed via
//! `code_context`'s own MCP evidence, reproduced faithfully in
//! [`run_code_agent_harness`] below — every field the Python reads from
//! `context` is read here too, no fewer, no more).
//!
//! That module does not exist yet in `v2/kl4a-rs`; this file compiles once
//! whichever batch ports `context.py` adds `pub mod context;` to `codekb`'s
//! `lib.rs` with a matching signature. See the handback report for the exact
//! ledger entry.

use std::path::Path;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::context::code_context;

/// Port of `kl4a.codekb.agent.run_code_agent_harness`.
///
/// `provider` defaults to `"fixture"` in Python (`provider: str = "fixture"`);
/// callers that want the Python default pass `"fixture"` explicitly here —
/// Rust has no keyword-default sugar to mirror one-for-one, so the default
/// is a documented convention rather than a language feature.
///
/// Python raises `ValueError(f"unsupported code agent provider: {provider}")`
/// for any other value; mirrored here as `anyhow::bail!` with the same
/// message, since this is the only branch in the whole module and does not
/// rise to the level of a `thiserror` variant other code branches on.
pub fn run_code_agent_harness(
    bundle_dir: &Path,
    task: &str,
    query: &str,
    provider: &str,
) -> Result<Value> {
    if provider != "fixture" {
        bail!("unsupported code agent provider: {provider}");
    }

    let context = code_context(bundle_dir, task, Some(query), None)?;

    let symbols = context.get("symbols").and_then(Value::as_array).cloned().unwrap_or_default();
    let relations = context.get("relations").and_then(Value::as_array).cloned().unwrap_or_default();
    let evidence = context.get("evidence").and_then(Value::as_array).cloned().unwrap_or_default();
    let knowledge = context.get("code_knowledge").and_then(Value::as_array).cloned().unwrap_or_default();

    let mut answer_lines = vec![
        format!("Task `{task}` was answered using the Code Knowledge Access Layer."),
        format!(
            "Retrieved {} symbol(s), {} relation(s), {} evidence span(s), and {} usable claim(s).",
            symbols.len(),
            relations.len(),
            evidence.len(),
            knowledge.len(),
        ),
    ];

    if let Some(first) = symbols.first() {
        let qname = first.get("qualified_name").and_then(Value::as_str).unwrap_or_default();
        let file = first.get("file").and_then(Value::as_str).unwrap_or_default();
        let line_start = first.get("line_start").cloned().unwrap_or(Value::Null);
        answer_lines.push(format!(
            "Primary symbol: `{qname}` at `{file}:{line_start}`."
        ));
    }

    if !relations.is_empty() {
        let exact_count = relations
            .iter()
            .filter(|relation| {
                relation.get("resolution_status").and_then(Value::as_str) == Some("exact")
            })
            .count();
        answer_lines.push(format!("Exact static relations available: {exact_count}."));
    }

    if !evidence.is_empty() {
        let handles: Vec<&str> = evidence
            .iter()
            .take(5)
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .collect();
        answer_lines.push(format!("Evidence handles: {}.", handles.join(", ")));
    }

    if let Some(warnings) = context.get("warnings").and_then(Value::as_array) {
        if !warnings.is_empty() {
            let joined: Vec<String> = warnings
                .iter()
                .map(|w| w.as_str().map(str::to_string).unwrap_or_else(|| w.to_string()))
                .collect();
            answer_lines.push(format!("Warnings: {}.", joined.join("; ")));
        }
    }

    let citations: Vec<Value> = evidence
        .iter()
        .filter_map(|item| item.get("id").cloned())
        .collect();

    Ok(json!({
        "provider": provider,
        "task": task,
        "query": query,
        "used_kal": true,
        "ui_dependency": false,
        "context": context,
        "answer": answer_lines.join("\n"),
        "citations": citations,
    }))
}
