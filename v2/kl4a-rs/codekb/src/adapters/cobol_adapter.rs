//! Port of `kl4a/codekb/adapters/cobol_adapter.py`.
//!
//! The bundle-facing COBOL adapter: reads `code_inventory.json` for COBOL
//! sources, dispatches each one to either the line-structured parser
//! (`parse_cobol_text`, the default / fallback) or an external-JSON-AST
//! parser (`parse_cobol_external_json`, used only when the bundle manifest
//! configures `codekb.languages.cobol.adapter: cobol-external-json` *and* a
//! `<path>.ast.json` sidecar exists next to the source), writes module/
//! symbol/evidence OKF docs, and separately mines PERFORM/CALL/READ/WRITE/
//! MOVE-TO/COMPUTE/IF relations out of each paragraph's/program's source
//! excerpt for `generate_cobol_relations`.
//!
//! Ported symbol-for-symbol from the Python source (verified via tools-code
//! MCP `code_symbols_get` against every function/constant in the file).
//! Dict-shaped Code Knowledge Bundle records (`source`, `module`, `symbol`,
//! `evidence`, `relation`) stay `serde_json::Value` per the shared
//! convention. `LogicalLine` (Python: a 2-key `{"line", "text"}` dict built
//! identically everywhere) and `ParsedTextResult` (the `{"modules",
//! "symbols", "evidence", "warnings", "backend"}` dict both text-parsers
//! return and both callers consume by the same fixed keys) are ported as
//! real structs instead.
//!
//! ## Cross-batch dependencies (out of scope for this batch, already present)
//! - `crate::ids::{code_module_id_for, code_symbol_id_for,
//!   code_evidence_id_for, code_relation_id_for}` — `v2/kl4a-rs/codekb/src/ids.rs`.
//! - `crate::state::{read_json, STATE_DIR}` — `v2/kl4a-rs/codekb/src/state.rs`
//!   (`STATE_DIR = ".codekb"`, confirmed by reading that file directly, not
//!   Python source).
//! - `crate::kl4a_shared::{write_markdown, load_manifest}` —
//!   `v2/kl4a-rs/codekb/src/kl4a_shared.rs`. NOTE: `model.rs` in this same crate
//!   instead calls `crate::bundle_store::load_manifest` (a module that does
//!   not exist yet) — that's a pre-existing wiring inconsistency from an
//!   earlier batch, not introduced here; flagged in the handback report.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};

use crate::ids::{code_evidence_id_for, code_module_id_for, code_relation_id_for, code_symbol_id_for};
use crate::bundle_store::load_manifest;
use crate::okf_writer::write_markdown;
use crate::state::{read_json, STATE_DIR};

/// Port of `kl4a.codekb.adapters.cobol_adapter.COBOL_KEYWORDS` (distinct
/// from, and a different set than, `cobol::PROCEDURE_KEYWORDS`).
static COBOL_KEYWORDS: Lazy<std::collections::HashSet<&'static str>> = Lazy::new(|| {
    [
        "IDENTIFICATION", "ENVIRONMENT", "DATA", "PROCEDURE", "DIVISION", "SECTION",
        "PROGRAM-ID", "WORKING-STORAGE", "FILE", "LINKAGE", "STOP", "RUN", "END", "IF", "ELSE",
        "EVALUATE", "WHEN", "MOVE", "COMPUTE", "READ", "WRITE", "OPEN", "CLOSE", "CALL",
        "PERFORM", "COPY",
    ]
    .into_iter()
    .collect()
});

// `re.match(...)`-derived patterns (Python's `match` anchors at the start of
// the string) carry an explicit `^` here so the unanchored-by-default
// `regex` crate behaves the same way. `re.finditer(...)`-derived patterns
// stay unanchored (they scan the whole haystack for every occurrence).
static PROGRAM_ID_LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^PROGRAM-ID\.\s+([A-Z0-9_-]+)\.").unwrap());
static COPY_LINE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^COPY\s+([A-Z0-9_-]+)\.").unwrap());
static DATA_LIKE_LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^(FD|01|05|10|15|77)\s+").unwrap());
static PERFORM_TEXT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bPERFORM\s+([A-Z0-9_-]+)").unwrap());
static CALL_TEXT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\bCALL\s+['"]?([A-Z0-9_-]+)"#).unwrap());
static IF_TEXT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bIF\s+(.+?)(?:\n|$)").unwrap());
static COMPUTE_TEXT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bCOMPUTE\s+([A-Z0-9_-]+)").unwrap());
static MOVE_TO_TEXT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bMOVE\s+.+?\s+TO\s+([A-Z0-9_-]+)").unwrap());

/// Mirrors Python's `x or default` truthiness fallback for an optional
/// string field: `None` *and* an empty string both fall through to
/// `default`, unlike a plain `Option::unwrap_or` (which only catches
/// `None`). Only used where the Python source actually wrote `... or ...`;
/// a plain `dict.get(key, default)` (presence-only fallback) is ported as
/// an ordinary `unwrap_or` instead.
fn or_str<'a>(value: Option<&'a str>, default: &'a str) -> &'a str {
    match value {
        Some(s) if !s.is_empty() => s,
        _ => default,
    }
}

/// `str(value)` for the handful of JSON scalar shapes Python's `str()` can
/// hit here (`ast_doc.get("imports", [])` entries are not guaranteed to be
/// strings).
fn value_to_display_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        other => other.to_string(),
    }
}

/// One entry of `kl4a.codekb.adapters.cobol_adapter.logical_cobol_lines`'s
/// return value (Python: `{"line": int, "text": str}`).
#[derive(Debug, Clone)]
pub struct LogicalLine {
    pub line: usize,
    pub text: String,
}

/// The dict shape both `parse_cobol_text` and `parse_cobol_external_json`
/// return, and both `parse_cobol_bundle` and `parse_cobol_source` consume
/// (`modules`, `symbols`, `evidence`, `warnings`, `backend`).
#[derive(Debug, Clone)]
pub struct ParsedTextResult {
    pub modules: Vec<Value>,
    pub symbols: Vec<Value>,
    pub evidence: Vec<Value>,
    pub warnings: Vec<Value>,
    pub backend: String,
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.parse_cobol_bundle`.
pub fn parse_cobol_bundle(bundle_dir: &Path) -> Result<Value> {
    let inventory = read_json(
        &bundle_dir.join(STATE_DIR).join("code_inventory.json"),
        json!({"sources": []}),
    );
    let backend = cobol_parser_backend(bundle_dir)?;

    let mut modules = Vec::new();
    let mut symbols = Vec::new();
    let mut evidence = Vec::new();
    let mut parser_runs = Vec::new();

    let empty_sources: Vec<Value> = Vec::new();
    let sources = inventory.get("sources").and_then(Value::as_array).unwrap_or(&empty_sources);
    for source in sources {
        if source.get("language").and_then(Value::as_str) != Some("cobol") {
            continue;
        }
        let original_path = source.get("original_path").and_then(Value::as_str).unwrap_or_default();
        let source_path = bundle_dir.join(original_path);
        let text = std::fs::read_to_string(&source_path)
            .with_context(|| format!("failed to read {}", source_path.display()))?;
        let parsed = parse_cobol_source(bundle_dir, source, &text, &backend)?;

        modules.extend(parsed.modules.iter().cloned());
        symbols.extend(parsed.symbols.iter().cloned());
        evidence.extend(parsed.evidence.iter().cloned());
        parser_runs.push(json!({
            "source_id": source.get("id"),
            "source_version_id": source.get("source_version_id"),
            "parser": parsed.backend,
            "parser_version": "0.1",
            "status": "parsed",
            "errors": Vec::<String>::new(),
            "warnings": parsed.warnings,
        }));

        for module in &parsed.modules {
            write_module_doc(bundle_dir, module)?;
        }
        for symbol in &parsed.symbols {
            let evidence_id = symbol.get("evidence_id").and_then(Value::as_str).unwrap_or_default();
            let symbol_evidence = parsed
                .evidence
                .iter()
                .find(|item| item["id"].as_str() == Some(evidence_id))
                .with_context(|| format!("no evidence entry for symbol evidence_id {evidence_id}"))?;
            write_symbol_doc(bundle_dir, symbol, symbol_evidence)?;
            write_evidence_doc(bundle_dir, symbol_evidence)?;
        }
    }

    Ok(json!({
        "modules": modules,
        "symbols": symbols,
        "evidence": evidence,
        "parser_runs": parser_runs,
    }))
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.cobol_parser_backend`.
pub fn cobol_parser_backend(bundle_dir: &Path) -> Result<String> {
    let manifest = load_manifest(bundle_dir)?;
    let codekb = manifest.get("codekb").filter(|v| !v.is_null());
    let languages = codekb.and_then(|c| c.get("languages")).filter(|v| !v.is_null());
    let cobol_config = languages
        .and_then(|l| l.get("cobol"))
        .filter(|v| !v.is_null())
        .cloned()
        .unwrap_or_else(|| json!({}));
    let adapter = or_str(
        cobol_config.get("adapter").and_then(Value::as_str),
        "cobol-line-structured",
    );
    Ok(adapter.to_string())
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.parse_cobol_source`.
pub fn parse_cobol_source(
    bundle_dir: &Path,
    source: &Value,
    text: &str,
    backend: &str,
) -> Result<ParsedTextResult> {
    if backend != "cobol-external-json" {
        return Ok(parse_cobol_text(source, text, backend));
    }

    if let Some(ast_path) = external_ast_path(bundle_dir, source) {
        if ast_path.exists() {
            let ast_doc = read_json(&ast_path, json!({}));
            return Ok(parse_cobol_external_json(source, text, &ast_doc, backend));
        }
    }

    let mut parsed = parse_cobol_text(source, text, "cobol-line-structured");
    let source_path = source.get("path").and_then(Value::as_str).unwrap_or_default();
    parsed.warnings.push(json!({
        "code": "external_ast_missing",
        "message": format!(
            "external AST not found for {source_path}; used line-structured fallback"
        ),
    }));
    Ok(parsed)
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.external_ast_path`.
pub fn external_ast_path(bundle_dir: &Path, source: &Value) -> Option<PathBuf> {
    let inventory = read_json(
        &bundle_dir.join(STATE_DIR).join("code_inventory.json"),
        json!({"repository": {}}),
    );
    let repo_path = inventory
        .get("repository")
        .and_then(|r| r.get("path"))
        .and_then(Value::as_str)?;
    if repo_path.is_empty() {
        return None;
    }
    let source_path = source.get("path").and_then(Value::as_str).unwrap_or_default();
    Some(PathBuf::from(repo_path).join(format!("{source_path}.ast.json")))
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.parse_cobol_external_json`.
pub fn parse_cobol_external_json(
    source: &Value,
    text: &str,
    ast_doc: &Value,
    backend: &str,
) -> ParsedTextResult {
    let stem_fallback = source
        .get("path")
        .and_then(Value::as_str)
        .map(|p| {
            Path::new(p)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string()
        })
        .unwrap_or_default();
    let module_name =
        or_str(ast_doc.get("program").and_then(Value::as_str), &stem_fallback).to_uppercase();
    let module_id = code_module_id_for(&module_name);

    let mut imports: Vec<String> = ast_doc
        .get("imports")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().map(|v| value_to_display_string(v).to_uppercase()).collect())
        .unwrap_or_default();
    imports.sort();

    let module = json!({
        "id": module_id,
        "title": module_name,
        "qualified_name": module_name,
        "source_id": source.get("id"),
        "source_version_id": source.get("source_version_id"),
        "file": source.get("path"),
        "language": "cobol",
        "imports": imports,
        "parser_backend": backend,
        "okf_path": format!("code/modules/{module_id}.md"),
    });

    let total_lines = text.lines().count().max(1);
    let root_kind = if source.get("kind").and_then(Value::as_str) == Some("copybook") {
        "copybook"
    } else {
        "program"
    };
    let mut symbols = vec![cobol_symbol(
        source,
        &module,
        &module_name,
        root_kind,
        1,
        total_lines,
        &format!("PROGRAM-ID. {module_name}."),
        backend,
    )];

    if let Some(nodes) = ast_doc.get("symbols").and_then(Value::as_array) {
        for node in nodes {
            if !node.is_object() {
                continue;
            }
            // Python: `int(node.get("line_start") or 1)` — a present-but-0
            // `line_start` is falsy too, so it also falls through to 1.
            let line_start_raw = node.get("line_start").and_then(Value::as_i64).unwrap_or(0);
            let line_start = if line_start_raw <= 0 { 1 } else { line_start_raw as usize };
            let line_end_raw = node.get("line_end").and_then(Value::as_i64).unwrap_or(0);
            let line_end = if line_end_raw <= 0 { line_start } else { line_end_raw as usize };

            let name = or_str(node.get("name").and_then(Value::as_str), "UNKNOWN").to_uppercase();
            let kind = or_str(node.get("kind").and_then(Value::as_str), "data_item").to_string();
            let name_field = node.get("name").and_then(Value::as_str).unwrap_or("");
            let signature_field = node.get("signature").and_then(Value::as_str).unwrap_or("");
            let signature = if !signature_field.is_empty() {
                signature_field.to_string()
            } else if !name_field.is_empty() {
                name_field.to_string()
            } else {
                "UNKNOWN".to_string()
            };

            symbols.push(cobol_symbol(
                source, &module, &name, &kind, line_start, line_end, &signature, backend,
            ));
        }
    }

    let evidence: Vec<Value> = symbols.iter().map(|s| evidence_record(source, s, text)).collect();
    ParsedTextResult {
        modules: vec![module],
        symbols,
        evidence,
        warnings: Vec::new(),
        backend: backend.to_string(),
    }
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.parse_cobol_text`.
pub fn parse_cobol_text(source: &Value, text: &str, backend: &str) -> ParsedTextResult {
    let lines = logical_cobol_lines(text);
    let program_name = program_name_for(source, &lines);
    let module_id = code_module_id_for(&program_name);
    let module = json!({
        "id": module_id,
        "title": program_name,
        "qualified_name": program_name,
        "source_id": source.get("id"),
        "source_version_id": source.get("source_version_id"),
        "file": source.get("path"),
        "language": "cobol",
        "imports": copybook_names(&lines),
        "parser_backend": backend,
        "okf_path": format!("code/modules/{module_id}.md"),
    });

    let total_lines = text.lines().count().max(1);
    let mut symbols: Vec<Value> = vec![cobol_symbol(
        source,
        &module,
        &program_name,
        "program",
        1,
        total_lines,
        &format!("PROGRAM-ID. {program_name}."),
        backend,
    )];

    for line in &lines {
        let upper = line.text.to_uppercase();
        if upper.ends_with(" SECTION.") {
            let name = upper.strip_suffix(" SECTION.").unwrap_or(&upper).trim().to_string();
            symbols.push(cobol_symbol(
                source,
                &module,
                &name,
                "section",
                line.line,
                next_control_line(&lines, line.line, text),
                &line.text,
                backend,
            ));
        } else if is_paragraph_line(&upper) {
            let name = upper.trim_end_matches('.').to_string();
            symbols.push(cobol_symbol(
                source,
                &module,
                &name,
                "paragraph",
                line.line,
                next_control_line(&lines, line.line, text),
                &line.text,
                backend,
            ));
        } else if upper.starts_with("COPY ") {
            let name = upper
                .strip_prefix("COPY ")
                .unwrap_or(&upper)
                .trim_end_matches('.')
                .trim()
                .to_string();
            symbols.push(cobol_symbol(
                source,
                &module,
                &name,
                "copy_statement",
                line.line,
                line.line,
                &line.text,
                backend,
            ));
        } else if DATA_LIKE_LINE_RE.is_match(&upper) {
            let name = data_item_name(&upper);
            let kind = if upper.starts_with("FD ") {
                "file"
            } else if upper.starts_with("01 ") {
                "record"
            } else {
                "data_item"
            };
            symbols.push(cobol_symbol(
                source, &module, &name, kind, line.line, line.line, &line.text, backend,
            ));
        }
    }

    let evidence: Vec<Value> = symbols.iter().map(|s| evidence_record(source, s, text)).collect();
    ParsedTextResult {
        modules: vec![module],
        symbols,
        evidence,
        warnings: Vec::new(),
        backend: backend.to_string(),
    }
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.logical_cobol_lines`.
pub fn logical_cobol_lines(text: &str) -> Vec<LogicalLine> {
    let mut records = Vec::new();
    for (index, raw_line) in text.lines().enumerate() {
        let stripped = raw_line.trim();
        if stripped.is_empty() || stripped.starts_with('*') {
            continue;
        }
        let chars: Vec<char> = raw_line.chars().collect();
        if chars.len() >= 7 {
            let indicator = chars[6];
            if indicator == '*' || indicator == '/' {
                continue;
            }
        }
        let content = if chars.len() > 6 {
            let prefix: String = chars[..6].iter().collect();
            let prefix_trimmed = prefix.trim();
            let is_digit_seq = !prefix_trimmed.is_empty() && prefix_trimmed.chars().all(|c| c.is_ascii_digit());
            if is_digit_seq {
                chars[6..].iter().collect::<String>().trim().to_string()
            } else {
                stripped.to_string()
            }
        } else {
            stripped.to_string()
        };
        if !content.is_empty() {
            records.push(LogicalLine { line: index + 1, text: content });
        }
    }
    records
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.program_name_for`.
pub fn program_name_for(source: &Value, lines: &[LogicalLine]) -> String {
    for line in lines {
        if let Some(caps) = PROGRAM_ID_LINE_RE.captures(&line.text) {
            return caps[1].to_uppercase();
        }
    }
    source
        .get("path")
        .and_then(Value::as_str)
        .map(|p| {
            Path::new(p)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_uppercase()
        })
        .unwrap_or_default()
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.is_paragraph_line`.
pub fn is_paragraph_line(upper_line: &str) -> bool {
    let without_trailing_dots = upper_line.trim_end_matches('.');
    if !upper_line.ends_with('.') || without_trailing_dots.contains(' ') {
        return false;
    }
    let is_digit_seq =
        !without_trailing_dots.is_empty() && without_trailing_dots.chars().all(|c| c.is_ascii_digit());
    !COBOL_KEYWORDS.contains(without_trailing_dots) && !is_digit_seq
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.data_item_name`.
pub fn data_item_name(upper_line: &str) -> String {
    let trimmed = upper_line.trim_end_matches('.');
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return "UNKNOWN".to_string();
    }
    if parts[0] == "FD" && parts.len() > 1 {
        return parts[1].to_string();
    }
    let is_digit_seq = !parts[0].is_empty() && parts[0].chars().all(|c| c.is_ascii_digit());
    if is_digit_seq && parts.len() > 1 {
        return parts[1].to_string();
    }
    parts[0].to_string()
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.copybook_names`.
pub fn copybook_names(lines: &[LogicalLine]) -> Vec<String> {
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for line in lines {
        if let Some(caps) = COPY_LINE_RE.captures(&line.text) {
            names.insert(caps[1].to_uppercase());
        }
    }
    names.into_iter().collect()
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.next_control_line`.
pub fn next_control_line(lines: &[LogicalLine], current_line: usize, text: &str) -> usize {
    for line in lines {
        if line.line <= current_line {
            continue;
        }
        let upper = line.text.to_uppercase();
        if upper.ends_with(" SECTION.") || is_paragraph_line(&upper) {
            return current_line.max(line.line.saturating_sub(1));
        }
    }
    text.lines().count()
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.cobol_symbol`.
#[allow(clippy::too_many_arguments)]
pub fn cobol_symbol(
    source: &Value,
    module: &Value,
    name: &str,
    kind: &str,
    line_start: usize,
    line_end: usize,
    signature: &str,
    backend: &str,
) -> Value {
    let module_qualified_name = module["qualified_name"].as_str().unwrap_or_default();
    let qualified_name = if kind != "program" {
        format!("{module_qualified_name}.{name}")
    } else {
        module_qualified_name.to_string()
    };
    let symbol_id = code_symbol_id_for(&qualified_name);
    json!({
        "id": symbol_id,
        "title": name,
        "kind": kind,
        "qualified_name": qualified_name,
        "module_id": module["id"],
        "module": module_qualified_name,
        "source_id": source.get("id"),
        "source_version_id": source.get("source_version_id"),
        "file": source.get("path"),
        "language": "cobol",
        "line_start": line_start,
        "line_end": line_end,
        "ast_node_type": format!("cobol.{kind}"),
        "signature": signature,
        "language_specific": { "cobol": { "parser_backend": backend } },
        "decorators": Vec::<String>::new(),
        "docstring": Value::Null,
        "okf_path": format!("code/symbols/{symbol_id}.md"),
        "evidence_id": code_evidence_id_for(&symbol_id),
    })
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.evidence_record`.
pub fn evidence_record(source: &Value, symbol: &Value, text: &str) -> Value {
    let lines: Vec<&str> = text.lines().collect();
    let start = (symbol["line_start"].as_u64().unwrap_or(1) as usize).max(1);
    let end = (symbol["line_end"].as_u64().unwrap_or(start as u64) as usize).max(start);
    let excerpt = if start - 1 < lines.len() {
        lines[(start - 1)..end.min(lines.len())].join("\n")
    } else {
        String::new()
    };
    let evidence_id = symbol.get("evidence_id").cloned().unwrap_or(Value::Null);
    json!({
        "id": evidence_id,
        "title": format!(
            "{} lines {}-{}",
            symbol["qualified_name"].as_str().unwrap_or_default(),
            start,
            end
        ),
        "source_id": source.get("id"),
        "source_version_id": source.get("source_version_id"),
        "symbol_id": symbol["id"],
        "file": source.get("path"),
        "language": "cobol",
        "line_start": start,
        "line_end": end,
        "ast_node_type": symbol["ast_node_type"],
        "span_status": "exact",
        "excerpt": excerpt,
        "okf_path": format!("evidence/{}.md", evidence_id.as_str().unwrap_or_default()),
    })
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.cobol_relation`.
pub fn cobol_relation(
    symbol: &Value,
    predicate: &str,
    object_value: &str,
    object_kind: &str,
    resolution_status: &str,
    confidence: f64,
) -> Value {
    let symbol_id = symbol.get("id").and_then(Value::as_str).unwrap_or_default();
    let relation_id = code_relation_id_for(symbol_id, predicate, object_value);
    json!({
        "id": relation_id,
        "type": "Code Knowledge Relation",
        "title": format!("{symbol_id} {predicate} {object_value}"),
        "subject": symbol_id,
        "predicate": predicate,
        "object": object_value,
        "subject_kind": "symbol",
        "object_kind": object_kind,
        "relation": {
            "rdf_compatible": true,
            "resolution_status": resolution_status,
            "resolver": "cobol-line-structured",
            "confidence": (confidence * 100.0).round() / 100.0,
        },
        "code": {
            "language": "cobol",
            "file": symbol.get("file").cloned().unwrap_or(Value::Null),
        },
        "evidence": [symbol.get("evidence_id").cloned().unwrap_or(Value::Null)],
        "okf_path": format!("relations/{relation_id}.md"),
    })
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.performed_names`.
pub fn performed_names(text: &str) -> Vec<String> {
    PERFORM_TEXT_RE.captures_iter(text).map(|c| c[1].to_uppercase()).collect()
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.called_names`.
pub fn called_names(text: &str) -> Vec<String> {
    CALL_TEXT_RE.captures_iter(text).map(|c| c[1].to_uppercase()).collect()
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.branch_conditions`.
pub fn branch_conditions(text: &str) -> Vec<String> {
    IF_TEXT_RE
        .captures_iter(text)
        .map(|c| c[1].trim().to_uppercase())
        .collect()
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.io_names`.
///
/// `verb` is always a compile-time-literal COBOL keyword ("READ"/"WRITE")
/// at every call site, so the dynamically-built pattern needs no
/// `regex::escape` for safety here, but it costs nothing to be defensive.
pub fn io_names(text: &str, verb: &str) -> Vec<String> {
    let pattern = format!(r"(?i)\b{}\s+([A-Z0-9_-]+)", regex::escape(verb));
    let re = Regex::new(&pattern).expect("io_names pattern is always valid");
    re.captures_iter(text).map(|c| c[1].to_uppercase()).collect()
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.updated_names`.
pub fn updated_names(text: &str) -> Vec<String> {
    let mut names: Vec<String> =
        COMPUTE_TEXT_RE.captures_iter(text).map(|c| c[1].to_uppercase()).collect();
    names.extend(MOVE_TO_TEXT_RE.captures_iter(text).map(|c| c[1].to_uppercase()));
    names
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.source_excerpt`.
pub fn source_excerpt(bundle_dir: &Path, symbol: &Value) -> Result<String> {
    let evidence_id = symbol.get("evidence_id").and_then(Value::as_str).unwrap_or_default();
    let evidence_path = bundle_dir.join("evidence").join(format!("{evidence_id}.md"));
    if evidence_path.exists() {
        return std::fs::read_to_string(&evidence_path)
            .with_context(|| format!("failed to read {}", evidence_path.display()));
    }
    Ok(or_str(symbol.get("signature").and_then(Value::as_str), "").to_string())
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.generate_cobol_relations`.
pub fn generate_cobol_relations(bundle_dir: &Path, symbols_state: &Value) -> Result<Vec<Value>> {
    let empty: Vec<Value> = Vec::new();
    let symbols: Vec<&Value> = symbols_state
        .get("symbols")
        .and_then(Value::as_array)
        .unwrap_or(&empty)
        .iter()
        .filter(|s| s.get("language").and_then(Value::as_str) == Some("cobol"))
        .collect();

    // Python also builds a `symbols_by_module` map here that is never read
    // again in the rest of the function — dead in the original too, so it
    // is not ported.
    let mut by_title: std::collections::HashMap<String, &Value> = std::collections::HashMap::new();
    for symbol in &symbols {
        let title = symbol.get("title").and_then(Value::as_str).unwrap_or_default().to_uppercase();
        by_title.entry(title).or_insert(symbol);
    }

    let mut relations = Vec::new();
    for symbol in &symbols {
        let kind = symbol.get("kind").and_then(Value::as_str).unwrap_or_default();
        if kind != "paragraph" && kind != "program" {
            continue;
        }
        let source_text = source_excerpt(bundle_dir, symbol)?;

        for target_name in performed_names(&source_text) {
            let target = by_title.get(&target_name);
            let (object_value, object_kind, resolution_status, confidence) = match target {
                Some(t) => (
                    t.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
                    "symbol",
                    "exact",
                    0.94,
                ),
                None => (target_name.clone(), "paragraph", "unresolved", 0.45),
            };
            relations.push(cobol_relation(symbol, "performs", &object_value, object_kind, resolution_status, confidence));
        }

        for target_name in called_names(&source_text) {
            let target = by_title.get(&target_name);
            let (object_value, object_kind, resolution_status, confidence) = match target {
                Some(t) => (
                    t.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
                    "symbol",
                    "exact",
                    0.94,
                ),
                None => (target_name.clone(), "external_program", "unresolved", 0.45),
            };
            relations.push(cobol_relation(symbol, "calls", &object_value, object_kind, resolution_status, confidence));
        }

        for file_name in io_names(&source_text, "READ") {
            relations.push(cobol_relation(symbol, "reads", &file_name, "file", "inferred", 0.72));
        }
        for file_name in io_names(&source_text, "WRITE") {
            relations.push(cobol_relation(symbol, "writes", &file_name, "file", "inferred", 0.72));
        }

        for item_name in updated_names(&source_text) {
            let target = by_title.get(&item_name);
            let (object_value, object_kind) = match target {
                Some(t) => (t.get("id").and_then(Value::as_str).unwrap_or_default().to_string(), "symbol"),
                None => (item_name.clone(), "data_item"),
            };
            relations.push(cobol_relation(symbol, "updates", &object_value, object_kind, "inferred", 0.72));
        }

        for condition in branch_conditions(&source_text) {
            relations.push(cobol_relation(symbol, "branches_on", &condition, "condition_expression", "exact", 0.9));
        }
    }
    Ok(relations)
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.write_module_doc`.
pub fn write_module_doc(bundle_dir: &Path, module: &Value) -> Result<()> {
    let okf_path = module.get("okf_path").and_then(Value::as_str).unwrap_or_default();
    let title = module.get("title").and_then(Value::as_str).unwrap_or_default();
    // Python: `module.get("parser_backend", "cobol-line-structured")` — a
    // presence-only fallback (`dict.get(key, default)`), not `or`, so a
    // plain `unwrap_or` (not `or_str`) is the right port here.
    let parser_backend = module
        .get("parser_backend")
        .and_then(Value::as_str)
        .unwrap_or("cobol-line-structured");
    let frontmatter = json!({
        "type": "Code Module",
        "title": title,
        "module_id": module.get("id"),
        "code": {
            "language": "cobol",
            "qualified_name": module.get("qualified_name"),
            "file": module.get("file"),
            "imports": module.get("imports"),
            "language_specific": { "cobol": { "parser_backend": parser_backend } },
        },
    });
    write_markdown(&bundle_dir.join(okf_path), &frontmatter, &format!("# {title}\n"))
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.write_symbol_doc`.
pub fn write_symbol_doc(bundle_dir: &Path, symbol: &Value, evidence: &Value) -> Result<()> {
    let okf_path = symbol.get("okf_path").and_then(Value::as_str).unwrap_or_default();
    let title = symbol.get("title").and_then(Value::as_str).unwrap_or_default();
    let qualified_name = symbol.get("qualified_name").and_then(Value::as_str).unwrap_or_default();
    let parser_backend = symbol
        .get("language_specific")
        .and_then(|ls| ls.get("cobol"))
        .and_then(|c| c.get("parser_backend"))
        .and_then(Value::as_str)
        .unwrap_or("cobol-line-structured");
    let frontmatter = json!({
        "type": "Code Symbol",
        "title": title,
        "symbol_id": symbol.get("id"),
        "code": {
            "language": "cobol",
            "kind": symbol.get("kind"),
            "qualified_name": qualified_name,
            "file": symbol.get("file"),
            "module": symbol.get("module"),
            "line_start": symbol.get("line_start"),
            "line_end": symbol.get("line_end"),
            "ast_node_type": symbol.get("ast_node_type"),
            "signature": symbol.get("signature"),
            "language_specific": { "cobol": { "parser_backend": parser_backend } },
        },
        "evidence": [evidence.get("id")],
    });
    write_markdown(
        &bundle_dir.join(okf_path),
        &frontmatter,
        &format!("# {title}\n\nQualified name: `{qualified_name}`\n"),
    )
}

/// Quotes a YAML plain scalar only when required (empty, leading/embedded
/// characters that are not safe unquoted, or a value that would otherwise
/// parse back as a different type). Used only by [`write_evidence_doc`]'s
/// hand-rolled frontmatter — see the comment there for why it can't route
/// through the shared `write_markdown`/`serde_json::Value` path.
fn yaml_plain_scalar(value: &str) -> String {
    let needs_quoting = value.is_empty()
        || value.starts_with(|c: char| "-?:,[]{}#&*!|>'\"%@`".contains(c))
        || value.contains(": ")
        || value.ends_with(':')
        || value != value.trim()
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "null" | "~" | "true" | "false" | "yes" | "no"
        )
        || value.parse::<f64>().is_ok();
    if needs_quoting {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

/// Port of `kl4a.codekb.adapters.cobol_adapter.write_evidence_doc`.
///
/// Unlike the other `write_*_doc` helpers in this file, this does NOT route
/// its frontmatter through the shared `write_markdown(&Path, &Value, &str)`
/// helper. That helper serializes its `frontmatter: &serde_json::Value` via
/// `serde_yaml::to_string`, and `serde_json::Value::Object` in this
/// workspace is backed by `BTreeMap` — `Cargo.lock`'s `serde_json` entry has
/// no `indexmap` dependency, confirming the `preserve_order` feature is not
/// enabled anywhere in the build — so its keys always come out sorted
/// alphabetically, regardless of the order fields are inserted in a `json!`
/// literal. Python's `write_markdown` instead calls
/// `yaml.safe_dump(frontmatter, sort_keys=False, ...)`, which preserves the
/// dict's real construction order: `type, title, evidence_id,
/// code{language, file, symbol_id, line_start, line_end, ast_node_type,
/// span_status}`.
///
/// That difference is externally visible: `source_excerpt` (below) later
/// reads this exact file back as raw text, and `branch_conditions` /
/// `performed_names` / `called_names` / `io_names` / `updated_names` regex
/// it. A paragraph symbol whose name happens to end in a bare COBOL keyword
/// (e.g. `END-IF`, `END-READ`) puts that keyword at the tail of a YAML
/// line, which is itself a valid `\bIF\b`/`\bREAD\b` regex trigger — so
/// *which* line's content gets swept into a spurious match depends on what
/// line follows in the frontmatter, i.e. on field order. With Python's
/// order (`... evidence_id: ...-end-if` immediately followed by `code:`,
/// and `... symbol_id: ...-END-IF` immediately followed by `line_start:
/// 25`), `branch_conditions` incidentally captures `CODE:` and
/// `LINE_START: 25` as extra `branches_on` objects. With alphabetical
/// order (`code` < `evidence_id` < `title` < `type`), the `code:` block's
/// last key `symbol_id: ...-END-IF` is instead followed by the top-level
/// `evidence_id: ...` line, so the alphabetically-sorted port previously
/// captured a bogus `EVIDENCE_ID: ...` object instead and dropped `CODE:`
/// / `LINE_START: 25` — the exact divergence this fixes.
///
/// Hand-rolling the frontmatter text here (rather than reordering the
/// `json!` call, which has no effect on `BTreeMap` iteration order) is the
/// only fix available from this file alone; `write_markdown` itself lives
/// in `kl4a-core::okf_writer`, outside this file's ownership.
pub fn write_evidence_doc(bundle_dir: &Path, evidence: &Value) -> Result<()> {
    let okf_path = evidence.get("okf_path").and_then(Value::as_str).unwrap_or_default();
    let title = evidence.get("title").and_then(Value::as_str).unwrap_or_default();
    let excerpt = evidence.get("excerpt").and_then(Value::as_str).unwrap_or_default();
    let evidence_id = evidence.get("id").and_then(Value::as_str).unwrap_or_default();
    let file = evidence.get("file").and_then(Value::as_str).unwrap_or_default();
    let symbol_id = evidence.get("symbol_id").and_then(Value::as_str).unwrap_or_default();
    let line_start = evidence.get("line_start").and_then(Value::as_i64).unwrap_or_default();
    let line_end = evidence.get("line_end").and_then(Value::as_i64).unwrap_or_default();
    let ast_node_type = evidence.get("ast_node_type").and_then(Value::as_str).unwrap_or_default();
    let span_status = evidence.get("span_status").and_then(Value::as_str).unwrap_or_default();

    let frontmatter_text = format!(
        "type: Code Evidence\ntitle: {}\nevidence_id: {}\ncode:\n  language: cobol\n  file: {}\n  symbol_id: {}\n  line_start: {}\n  line_end: {}\n  ast_node_type: {}\n  span_status: {}",
        yaml_plain_scalar(title),
        yaml_plain_scalar(evidence_id),
        yaml_plain_scalar(file),
        yaml_plain_scalar(symbol_id),
        line_start,
        line_end,
        yaml_plain_scalar(ast_node_type),
        yaml_plain_scalar(span_status),
    );
    let body = format!("# {title}\n\n```cobol\n{excerpt}\n```\n");
    let path = bundle_dir.join(okf_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let text = format!("---\n{frontmatter_text}\n---\n\n{body}");
    std::fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))
}
