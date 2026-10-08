//! Port of `kl4a/codekb/mcp.py`.
//!
//! MCP (Model Context Protocol) tool catalogue and JSON-RPC dispatcher for a
//! Code Knowledge Bundle. Ported symbol-for-symbol from the Python source
//! (verified via tools-code MCP `code_symbols_get`); every function/constant
//! below has a 1:1 Python counterpart cited in its doc comment.
//!
//! ## Cross-batch dependencies (NOT part of this batch — report only)
//!
//! `call_mcp_tool` dispatches to `kl4a.codekb.context`'s query surface, which
//! is not in this batch. Expected here as `crate::context::{code_repo_describe,
//! code_files_search, code_symbols_search, code_symbol_get,
//! code_relations_search, code_relation_neighborhood, code_tests_for_symbol,
//! code_change_impact, code_context}`, each taking `bundle_dir: &Path` plus
//! the tool's own arguments and returning `anyhow::Result<serde_json::Value>`
//! (mirroring the `Any` JSON-serializable payloads the Python versions
//! return). This file compiles once that module lands with matching
//! signatures — see the handback report for the exact ledger entry.

use std::io::{BufRead, Write};
use std::path::Path;

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Map, Value};

use crate::context;

/// Port of `kl4a.codekb.mcp.SERVER_INSTRUCTIONS`.
pub const SERVER_INSTRUCTIONS: &str = "Answer questions about this code repository only from the grounded Code Knowledge Bundle exposed by these tools. Every claim carries evidence; cite it rather than answering from general knowledge.";

/// Port of `kl4a.codekb.mcp.object_schema`.
///
/// `properties` is an ordered list of `(name, json_type)` pairs (Python's
/// `dict[str, str]`, but preserved as a `Vec` here since JSON object key
/// order is otherwise unspecified and the Python dict-literal call sites
/// have a fixed, meaningful order).
pub fn object_schema(properties: &[(&str, &str)], required: &[&str]) -> Value {
    let mut props = Map::new();
    for (name, type_name) in properties {
        props.insert((*name).to_string(), json!({ "type": type_name }));
    }
    json!({
        "type": "object",
        "properties": Value::Object(props),
        "required": required,
        "additionalProperties": false,
    })
}

/// Port of `kl4a.codekb.mcp.MCP_TOOLS`.
pub fn mcp_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "code.repo.describe",
            "description": "Describe a Code Knowledge Bundle.",
            "inputSchema": object_schema(&[], &[]),
        }),
        json!({
            "name": "code.files.search",
            "description": "Search code source files.",
            "inputSchema": object_schema(&[("query", "string"), ("language", "string")], &["query"]),
        }),
        json!({
            "name": "code.symbols.search",
            "description": "Search code symbols.",
            "inputSchema": object_schema(&[("query", "string"), ("language", "string")], &["query"]),
        }),
        json!({
            "name": "code.symbols.get",
            "description": "Get one code symbol with evidence and relations.",
            "inputSchema": object_schema(&[("symbol_id", "string")], &["symbol_id"]),
        }),
        json!({
            "name": "code.relations.search",
            "description": "Search code relations.",
            "inputSchema": object_schema(
                &[
                    ("subject", "string"),
                    ("predicate", "string"),
                    ("object", "string"),
                    ("resolution_status", "string"),
                ],
                &[],
            ),
        }),
        json!({
            "name": "code.callgraph.neighborhood",
            "description": "Get a symbol/module relation neighborhood.",
            "inputSchema": object_schema(&[("node_id", "string"), ("depth", "integer")], &["node_id"]),
        }),
        json!({
            "name": "code.tests.for_symbol",
            "description": "List tests related to a symbol.",
            "inputSchema": object_schema(&[("symbol_id", "string")], &["symbol_id"]),
        }),
        json!({
            "name": "code.change_impact",
            "description": "Estimate direct code change impact.",
            "inputSchema": object_schema(&[("symbol_id", "string")], &["symbol_id"]),
        }),
        json!({
            "name": "code.context",
            "description": "Get task-ready code context for a developer agent.",
            "inputSchema": object_schema(
                &[("task", "string"), ("query", "string"), ("language", "string")],
                &["task"],
            ),
        }),
    ]
}

/// Port of `kl4a.codekb.mcp.require_arg`.
///
/// Python raises `ValueError(f"missing required argument: {name}")` when the
/// argument is absent, not a string, or blank after stripping; mirrored here
/// as an `Err`.
pub fn require_arg(arguments: &Map<String, Value>, name: &str) -> Result<String> {
    match arguments.get(name).and_then(Value::as_str) {
        Some(value) if !value.trim().is_empty() => Ok(value.to_string()),
        _ => bail!("missing required argument: {name}"),
    }
}

fn opt_str<'a>(arguments: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    arguments.get(name).and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// Port of `kl4a.codekb.mcp.list_mcp_tools`.
pub fn list_mcp_tools() -> Vec<Value> {
    mcp_tools()
}

/// Port of `kl4a.codekb.mcp.call_mcp_tool`.
///
/// Dispatches on `name` to the matching `context::*` query function. Python
/// raises `KeyError(f"unknown tool: {name}")` for an unrecognized name;
/// mirrored here as an `Err`. Every branch of `MCP_TOOLS` is represented —
/// this is a generic dispatch table in Python only by convention (a chain of
/// `if name == ...: return ...`), so each tool name gets its own match arm
/// here rather than a data-driven lookup, matching the original 1:1.
pub fn call_mcp_tool(
    bundle_dir: &Path,
    name: &str,
    arguments: Option<&Map<String, Value>>,
) -> Result<Value> {
    let empty = Map::new();
    let args = arguments.unwrap_or(&empty);
    match name {
        "code.repo.describe" => context::code_repo_describe(bundle_dir),
        "code.files.search" => {
            context::code_files_search(bundle_dir, &require_arg(args, "query")?, opt_str(args, "language"))
        }
        "code.symbols.search" => {
            context::code_symbols_search(bundle_dir, &require_arg(args, "query")?, opt_str(args, "language"))
        }
        "code.symbols.get" => context::code_symbol_get(bundle_dir, &require_arg(args, "symbol_id")?),
        "code.relations.search" => context::code_relations_search(
            bundle_dir,
            opt_str(args, "subject").unwrap_or(""),
            opt_str(args, "predicate").unwrap_or(""),
            opt_str(args, "object").unwrap_or(""),
            opt_str(args, "resolution_status").unwrap_or(""),
        ),
        "code.callgraph.neighborhood" => {
            let depth = args.get("depth").and_then(Value::as_i64).unwrap_or(1);
            context::code_relation_neighborhood(bundle_dir, &require_arg(args, "node_id")?, depth)
        }
        "code.tests.for_symbol" => context::code_tests_for_symbol(bundle_dir, &require_arg(args, "symbol_id")?),
        "code.change_impact" => context::code_change_impact(bundle_dir, &require_arg(args, "symbol_id")?),
        "code.context" => context::code_context(
            bundle_dir,
            &require_arg(args, "task")?,
            opt_str(args, "query"),
            opt_str(args, "language"),
        ),
        other => bail!("unknown tool: {other}"),
    }
}

/// Port of `kl4a.codekb.mcp.SUPPORTED_PROTOCOL_VERSIONS`.
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// Port of `kl4a.codekb.mcp.DEFAULT_PROTOCOL_VERSION`.
///
/// `SUPPORTED_PROTOCOL_VERSIONS[-1]` in Python — the last (oldest-listed)
/// entry, not the first.
pub fn default_protocol_version() -> &'static str {
    SUPPORTED_PROTOCOL_VERSIONS[SUPPORTED_PROTOCOL_VERSIONS.len() - 1]
}

/// Port of `kl4a.codekb.mcp.handle_jsonrpc_request`.
///
/// Returns `Ok(None)` for a `notifications/*` method, matching Python's
/// `return None` (no response is written for a notification). Every
/// method branch from the Python `if/elif` chain is represented:
/// `initialize`, `notifications/*`, `ping`, `tools/list`, `tools/call`, and
/// the trailing `else: raise KeyError(...)`. An exception anywhere in the
/// body is caught and turned into a JSON-RPC error response with code
/// `-32000`, exactly as Python's `except Exception as exc` does — mirrored
/// here by catching `call_mcp_tool`'s `Err` (and `require_arg`'s) rather
/// than letting it propagate.
pub fn handle_jsonrpc_request(bundle_dir: &Path, request: &Value) -> Result<Option<Value>> {
    let request_id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str);
    let params = request
        .get("params")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let outcome: Result<Value> = (|| {
        match method {
            Some("initialize") => {
                let requested = params.get("protocolVersion").and_then(Value::as_str);
                let protocol_version = match requested {
                    Some(v) if SUPPORTED_PROTOCOL_VERSIONS.contains(&v) => v,
                    _ => default_protocol_version(),
                };
                Ok(json!({
                    "protocolVersion": protocol_version,
                    "serverInfo": {"name": "codekb", "version": "0.0.2"},
                    "capabilities": {"tools": {}},
                }))
            }
            Some(m) if m.starts_with("notifications/") => {
                // Signalled to the caller via the Ok(None) branch below.
                Ok(Value::Null)
            }
            Some("ping") => Ok(json!({"ok": true})),
            Some("tools/list") => Ok(json!({"tools": list_mcp_tools()})),
            Some("tools/call") => {
                let name = require_arg(&params, "name")?;
                let arguments = params.get("arguments").and_then(Value::as_object).cloned();
                let result = call_mcp_tool(bundle_dir, &name, arguments.as_ref())?;
                let text = serde_json::to_string_pretty(&sort_keys(&result))
                    .map_err(|e| anyhow!(e))?;
                Ok(json!({"content": [{"type": "text", "text": text}]}))
            }
            Some(other) => bail!("unknown JSON-RPC method: {other}"),
            None => bail!("unknown JSON-RPC method: None"),
        }
    })();

    // A `notifications/*` method returns no response at all (matches
    // Python's early `return None`), distinct from an error response.
    if let Some(m) = method {
        if m.starts_with("notifications/") {
            return Ok(None);
        }
    }

    match outcome {
        Ok(result) => Ok(Some(
            json!({"jsonrpc": "2.0", "id": request_id, "result": result}),
        )),
        Err(exc) => Ok(Some(json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {"code": -32000, "message": exc.to_string()},
        }))),
    }
}

/// Recursively sorts object keys so JSON serialization matches Python's
/// `json.dumps(..., sort_keys=True)`. `serde_json::Value`'s default `Map` is
/// already insertion-ordered (not sorted), so this normalizes it.
fn sort_keys(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
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

/// Port of `kl4a.codekb.mcp.serve_mcp_stdio`.
///
/// Reads newline-delimited JSON-RPC requests from `stdin`, dispatches each
/// through [`handle_jsonrpc_request`], and writes any non-`None` response as
/// a single sorted-keys JSON line to `stdout`, flushing after every write —
/// matching Python's `for line in input_stream: ... output_stream.flush()`
/// loop exactly, including skipping blank lines.
pub fn serve_mcp_stdio(
    bundle_dir: &Path,
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
) -> Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let read = stdin.read_line(&mut line)?;
        if read == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = serde_json::from_str(&line)?;
        if let Some(response) = handle_jsonrpc_request(bundle_dir, &request)? {
            let text = serde_json::to_string(&sort_keys(&response)).map_err(|e| anyhow!(e))?;
            writeln!(stdout, "{text}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}
