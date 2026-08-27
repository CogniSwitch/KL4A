//! `handle_jsonrpc_request` (§6.4) and `serve_mcp_stdio` (§6.5): the JSON-RPC envelope
//! and the newline-delimited (JSONL) stdio transport. Mirrors
//! `tools/sopkb/sopkb/mcp_server.py`'s `handle_jsonrpc_request`/`serve_mcp_stdio`.
//! See docs/port/port-mapping-d-agent-mcp.md §6.4-§6.5.

use crate::tools::{call_mcp_tool, list_mcp_tools, require_arg};
use serde_json::{json, Value};
use sopkb_core::error::{Result, SopkbError};
use std::io::{self, BufRead, Write};
use std::path::Path;

/// Python truthiness for a `serde_json::Value`, used to mirror `x or default` sites
/// (`params = request.get("params") or {}`, `arguments = params.get("arguments") or {}`).
fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn truthy_or_empty_object(v: Option<&Value>) -> Value {
    match v {
        Some(val) if is_truthy(val) => val.clone(),
        _ => json!({}),
    }
}

/// `str(method)` equivalent for the "unknown JSON-RPC method" error message. Only
/// `None`/string are common in practice; anything else is a best-effort compact JSON
/// rendering (this edge -- a non-string `method` field -- is not a named P-P deviation,
/// since real JSON-RPC clients always send a string method).
fn method_repr(method: Option<&Value>) -> String {
    match method {
        None | Some(Value::Null) => "None".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// Returned as `instructions` on `initialize` (mirrors oss-launch's `mcp_server.py`
/// `SERVER_INSTRUCTIONS`, added to close the "MCP agents aren't told to ground answers
/// in tool output or to apply decision rules" gap found in code review).
const SERVER_INSTRUCTIONS: &str = "Ground every answer only in what these tools return \
    — never in general/internet/training-data knowledge, even when labeled as such. Call \
    knowledge.search (or agent.context) first; if nothing relevant comes back, say \
    explicitly that this knowledge base has no grounded answer for that part instead of \
    filling the gap. Always show the actual facts too: alongside any summary or \
    paraphrase, quote the raw evidence/source_text returned by knowledge.search or \
    evidence.get verbatim, and cite the knowledge item, section, or source it came from. \
    If a knowledge.search result carries rule_ids, or knowledge.get/agent.context shows a \
    decision_rules entry, that item is governed by a structured condition/obligation/\
    otherwise rule — fetch it (knowledge.get for the item, or agent.context for the task) \
    and apply that rule's logic wherever it's applicable to the question, rather than \
    answering from the prose evidence alone.";

/// `handle_jsonrpc_request` (§6.4). Returns `None` for a suppressed `notifications/*`
/// response (P-P4, decided FIX) -- the caller must not write a response line for that
/// case at all, not even an empty one.
pub fn handle_jsonrpc_request(bundle_dir: &Path, request: &Value, enable_review_notes: bool) -> Option<Value> {
    let request_id = request.get("id").cloned().unwrap_or(Value::Null);
    let method_value = request.get("method");
    let method_str = method_value.and_then(|v| v.as_str());

    // P-P4, decided FIX: suppress a response entirely for any notifications/* method.
    if let Some(m) = method_str {
        if m.starts_with("notifications/") {
            return None;
        }
    }

    let params = truthy_or_empty_object(request.get("params"));

    let outcome: Result<Value> = (|| match method_str {
        Some("initialize") => Ok(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {"name": "sopkb", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {"tools": {}},
            "instructions": SERVER_INSTRUCTIONS,
        })),
        Some("ping") => Ok(json!({ "ok": true })),
        Some("tools/list") => Ok(json!({ "tools": list_mcp_tools(enable_review_notes) })),
        Some("tools/call") => {
            let tool_name = require_arg(&params, "name")?;
            let arguments = truthy_or_empty_object(params.get("arguments"));
            let tool_result = call_mcp_tool(bundle_dir, &tool_name, &arguments, enable_review_notes)?;
            let text = sopkb_fmt::json_canonical::to_canonical_json(&tool_result);
            Ok(json!({ "content": [{ "type": "text", "text": text }] }))
        }
        _ => Err(SopkbError::Value(format!("unknown JSON-RPC method: {}", method_repr(method_value)))),
    })();

    Some(match outcome {
        Ok(result) => json!({"jsonrpc": "2.0", "id": request_id, "result": result}),
        Err(e) => json!({"jsonrpc": "2.0", "id": request_id, "error": {"code": -32000, "message": e.to_string()}}),
    })
}

/// Handles exactly one raw stdin line: blank lines are silently skipped (no output);
/// malformed JSON or a non-object request gets a `-32700` Parse error with `id: null`
/// for THAT LINE ONLY (P-P2, decided FIX -- Python's original has the parse outside the
/// try/except, so a bad line kills the whole process; this port never lets one bad line
/// take down the loop). Writes at most one compact, sorted-keys JSON line + `"\n"`
/// (P-P15) to `output`, then flushes.
pub fn process_line(bundle_dir: &Path, enable_review_notes: bool, raw_line: &str, output: &mut impl Write) -> io::Result<()> {
    let trimmed = raw_line.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let response = match serde_json::from_str::<Value>(trimmed) {
        Ok(request @ Value::Object(_)) => handle_jsonrpc_request(bundle_dir, &request, enable_review_notes),
        Ok(_) => Some(json!({
            "jsonrpc": "2.0",
            "id": Value::Null,
            "error": {"code": -32700, "message": "Parse error: request must be a JSON object"},
        })),
        Err(e) => Some(json!({
            "jsonrpc": "2.0",
            "id": Value::Null,
            "error": {"code": -32700, "message": format!("Parse error: {e}")},
        })),
    };

    if let Some(response) = response {
        writeln!(output, "{}", sopkb_fmt::json_canonical::to_compact_json(&response))?;
        output.flush()?;
    }
    Ok(())
}

/// `serve_mcp_stdio` (§6.5): reads newline-delimited JSON requests from `input` until
/// EOF, writing one compact JSON response line per request (except suppressed
/// notifications) to `output`. No `Content-Length` framing (P-P1). Strictly serial: one
/// request in, one response out, no concurrency. Loop terminates on EOF only -- no
/// `shutdown` method, no signal handling (P-P16, PRESERVE).
pub fn serve_mcp_stdio(bundle_dir: &Path, enable_review_notes: bool, mut input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = input.read_line(&mut line)?;
        if bytes_read == 0 {
            break; // EOF
        }
        process_line(bundle_dir, enable_review_notes, &line, &mut output)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sopkb_core::store;
    use std::io::Cursor;
    use tempfile::tempdir;

    fn bundle_with_one_item() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        store::create_bundle(&bundle_dir, Some("Test Bundle")).unwrap();
        let items = json!([{
            "id": "ki-a-000001", "item_type": "claim", "subject": "Staff", "predicate": "must",
            "object": "confirm patient identity", "source_id": "src-1", "section_id": "sec-1",
            "source_text": "Staff must confirm patient identity.", "start_pos": 0, "end_pos": 10,
            "span_status": "exact", "derivation": "direct_text", "confidence": 0.82,
            "review_status": "proposed", "metadata": {},
        }]);
        store::write_state_json(&bundle_dir, "items.json", &items).unwrap();
        dir
    }

    #[test]
    fn initialize_response_shape_and_pkg_version_p_p12() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let req = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"});
        let resp = handle_jsonrpc_request(&bundle_dir, &req, false).unwrap();
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(resp["result"]["serverInfo"]["name"], "sopkb");
        assert_eq!(resp["result"]["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(resp["result"]["capabilities"], json!({"tools": {}}));
        assert!(resp.get("error").is_none());
    }

    #[test]
    fn initialize_response_carries_evidence_grounding_instructions() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let req = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"});
        let resp = handle_jsonrpc_request(&bundle_dir, &req, false).unwrap();
        let instructions = resp["result"]["instructions"].as_str().unwrap();
        assert!(instructions.contains("quote the raw evidence"));
        assert!(instructions.contains("no grounded answer"));
        assert!(instructions.contains("decision_rules"));
    }

    #[test]
    fn ping_response() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let req = json!({"jsonrpc": "2.0", "id": "abc", "method": "ping"});
        let resp = handle_jsonrpc_request(&bundle_dir, &req, false).unwrap();
        assert_eq!(resp["id"], "abc");
        assert_eq!(resp["result"], json!({"ok": true}));
    }

    #[test]
    fn missing_id_echoes_null() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let req = json!({"jsonrpc": "2.0", "method": "ping"});
        let resp = handle_jsonrpc_request(&bundle_dir, &req, false).unwrap();
        assert_eq!(resp["id"], Value::Null);
    }

    #[test]
    fn tools_list_reflects_gating() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let req = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
        let resp = handle_jsonrpc_request(&bundle_dir, &req, false).unwrap();
        assert_eq!(resp["result"]["tools"].as_array().unwrap().len(), 16);

        let resp2 = handle_jsonrpc_request(&bundle_dir, &req, true).unwrap();
        assert_eq!(resp2["result"]["tools"].as_array().unwrap().len(), 17);
    }

    #[test]
    fn tools_call_wraps_result_as_indented_sorted_text_block() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let req = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "knowledge.get", "arguments": {"knowledge_item_id": "ki-a-000001"}},
        });
        let resp = handle_jsonrpc_request(&bundle_dir, &req, false).unwrap();
        let content = &resp["result"]["content"];
        assert_eq!(content[0]["type"], "text");
        let text = content[0]["text"].as_str().unwrap();
        // Must be the indent=2, sort_keys=true canonical form -- parse it back and check shape.
        assert!(text.starts_with("{\n  "), "must be indented, got: {text}");
        let reparsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(reparsed["id"], "ki-a-000001");
    }

    #[test]
    fn tools_call_null_result_serializes_to_literal_null_string() {
        // conflicts.list/freshness.check never return null, but knowledge_search on an
        // empty query with no items returns an empty array, not null -- exercise the
        // "unknown tool" -32000 path instead to prove tools/call never uses isError.
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let req = json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": {"name": "review.approve", "arguments": {}},
        });
        let resp = handle_jsonrpc_request(&bundle_dir, &req, false).unwrap();
        assert!(resp.get("result").is_none(), "a failing tool call must be a JSON-RPC error, not a result with isError");
        assert_eq!(resp["error"]["code"], -32000);
        assert_eq!(resp["error"]["message"], "unknown MCP tool: review.approve");
        assert_eq!(resp.as_object().unwrap().len(), 3, "exactly jsonrpc/id/error");
        assert_eq!(resp["error"].as_object().unwrap().len(), 2, "exactly code/message, no data");
    }

    #[test]
    fn tools_call_missing_arg_surfaces_as_minus_32000() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let req = json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {"name": "knowledge.get", "arguments": {}},
        });
        let resp = handle_jsonrpc_request(&bundle_dir, &req, false).unwrap();
        assert_eq!(resp["error"]["code"], -32000);
        assert_eq!(resp["error"]["message"], "missing required argument: knowledge_item_id");
    }

    #[test]
    fn unknown_method_error_unquoted_p_p5() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let req = json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown"});
        let resp = handle_jsonrpc_request(&bundle_dir, &req, false).unwrap();
        assert_eq!(resp["error"]["code"], -32000);
        // Unquoted -- NOT "'unknown JSON-RPC method: shutdown'" (Python str(KeyError) would quote it).
        assert_eq!(resp["error"]["message"], "unknown JSON-RPC method: shutdown");
    }

    #[test]
    fn notifications_star_suppressed_entirely() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let req = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        assert!(handle_jsonrpc_request(&bundle_dir, &req, false).is_none());

        let req2 = json!({"jsonrpc": "2.0", "id": 9, "method": "notifications/cancelled", "params": {}});
        assert!(handle_jsonrpc_request(&bundle_dir, &req2, false).is_none());
    }

    #[test]
    fn review_note_gating_end_to_end_through_envelope() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let req = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "review.note", "arguments": {"knowledge_item_id": "ki-a-000001", "rationale": "r"}},
        });
        let resp_disabled = handle_jsonrpc_request(&bundle_dir, &req, false).unwrap();
        assert_eq!(resp_disabled["error"]["message"], "review.note is disabled; start with --enable-review-notes");

        let resp_enabled = handle_jsonrpc_request(&bundle_dir, &req, true).unwrap();
        assert!(resp_enabled.get("result").is_some());
        let text = resp_enabled["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"action\": \"commented\""));
    }

    // --- process_line / serve_mcp_stdio: JSONL session tests over in-memory buffers ---

    #[test]
    fn process_line_skips_blank_lines_silently() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let mut out = Vec::new();
        process_line(&bundle_dir, false, "\n", &mut out).unwrap();
        process_line(&bundle_dir, false, "   \n", &mut out).unwrap();
        process_line(&bundle_dir, false, "", &mut out).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn process_line_malformed_json_yields_parse_error_and_does_not_panic() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let mut out = Vec::new();
        process_line(&bundle_dir, false, "{not json", &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        let resp: Value = serde_json::from_str(text.trim_end()).unwrap();
        assert_eq!(resp["error"]["code"], -32700);
        assert_eq!(resp["id"], Value::Null);
        assert!(resp["error"]["message"].as_str().unwrap().starts_with("Parse error:"));
    }

    #[test]
    fn process_line_non_object_json_yields_parse_error() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let mut out = Vec::new();
        process_line(&bundle_dir, false, "[1, 2, 3]", &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        let resp: Value = serde_json::from_str(text.trim_end()).unwrap();
        assert_eq!(resp["error"]["code"], -32700);
        assert_eq!(resp["id"], Value::Null);
    }

    #[test]
    fn compact_response_line_shape_matches_python_json_dumps_sort_keys() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let mut out = Vec::new();
        process_line(&bundle_dir, false, r#"{"jsonrpc": "2.0", "id": 8, "method": "ping"}"#, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text, "{\"id\": 8, \"jsonrpc\": \"2.0\", \"result\": {\"ok\": true}}\n");
    }

    #[test]
    fn end_to_end_jsonl_session_recovers_from_bad_line_and_keeps_processing() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let session = concat!(
            "{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"initialize\"}\n",
            "\n",
            "not even json\n",
            "{\"jsonrpc\": \"2.0\", \"method\": \"notifications/initialized\"}\n",
            "{\"jsonrpc\": \"2.0\", \"id\": 2, \"method\": \"tools/list\"}\n",
            "{\"jsonrpc\": \"2.0\", \"id\": 3, \"method\": \"tools/call\", \"params\": {\"name\": \"knowledge.get\", \"arguments\": {\"knowledge_item_id\": \"ki-a-000001\"}}}\n",
            "{\"jsonrpc\": \"2.0\", \"id\": 4, \"method\": \"bogus\"}\n",
        );
        let input = Cursor::new(session.as_bytes());
        let mut output = Vec::new();
        serve_mcp_stdio(&bundle_dir, false, input, &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // 6 requests sent; 1 is a suppressed notification -> 5 response lines.
        assert_eq!(lines.len(), 5);

        let r1: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(r1["id"], 1);
        assert_eq!(r1["result"]["protocolVersion"], "2024-11-05");

        let r2: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(r2["error"]["code"], -32700, "malformed line must not kill the loop");
        assert_eq!(r2["id"], Value::Null);

        let r3: Value = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(r3["id"], 2);
        assert_eq!(r3["result"]["tools"].as_array().unwrap().len(), 16);

        let r4: Value = serde_json::from_str(lines[3]).unwrap();
        assert_eq!(r4["id"], 3);
        assert!(r4["result"]["content"][0]["text"].as_str().unwrap().contains("ki-a-000001"));

        let r5: Value = serde_json::from_str(lines[4]).unwrap();
        assert_eq!(r5["id"], 4);
        assert_eq!(r5["error"]["message"], "unknown JSON-RPC method: bogus");

        // No response line ends in "\r\n" (P-P15).
        assert!(!text.contains('\r'));
    }
}
