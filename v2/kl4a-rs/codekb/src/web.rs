//! Port of `kl4a/codekb/web.py` -- HTML rendering + form handling for the
//! codekb bundle web UI. Every symbol below is grounded against the Python
//! source via tools-code MCP `code_symbols_get` excerpts, with one exception
//! (`docs_rail`, flagged at its definition) blocked by a genuine id collision
//! in the MCP index between `_DOCS_RAIL` (the constant) and `_docs_rail`
//! (the function): every `code_symbols_get` call for that id returns the
//! constant's evidence, never the function's own source, even though
//! `code_symbols_search` lists them as two distinct records sharing one id.
//!
//! WIRING (see also the batch report handed back to the coordinator):
//! - `crate::pipeline::{configured_mining, configured_repo_root, rebuild_risk,
//!   start_code_bundle_build}`, `crate::context::code_context`,
//!   `crate::render::CodeRenderer` and `kl4a::llm_settings` do not exist yet
//!   under `v2/kl4a-rs/` as of this batch (no `pipeline.rs`/`context.rs`/
//!   `render.rs`, and no sibling `kl4a` crate for `llm_settings` was found).
//!   Signatures below are inferred from how `web.py` calls them, not grounded
//!   against a Rust port of those modules.
//! - `crate::model` has no public cache-invalidation function. Python's
//!   `handle_code_pipeline_form`/`handle_code_review_form` both call
//!   `_CACHE.pop(str(bundle_dir), None)` after a mutation so the next page
//!   load does not serve a stale cached bundle; `model.rs`'s `CACHE` static
//!   is private with no equivalent exported. This port calls
//!   `crate::model::invalidate_code_bundle_cache(bundle_dir)`, which needs to
//!   be added to `model.rs` for this to compile and behave correctly.
//! - `server.rs` (already landed) imports `code_nav_items`, `code_active_view`
//!   and `render_code_page` from `crate::web` and calls
//!   `crate::layout::layout(&code_active_view(view), &describe, &body,
//!   message, &nav_views)` with 5 positional args, but the real `layout.rs`
//!   (also already landed) declares `layout(active, summary, body, message,
//!   base_path, nav_views: Option<&[NavView]>, searchable: bool)` — 7 args,
//!   a different `nav_views` type (`Option<&[NavView]>`, not a bare slice)
//!   and no direct 5-arg overload. That mismatch is between two files this
//!   batch does not own (server.rs and layout.rs); it is called out here
//!   only because this file's `code_nav_items` return type is one side of
//!   it. `code_nav_items` here returns `Vec<(String, String)>`, the literal
//!   Rust shape of Python's `list[tuple[str, str]]` return -- the most
//!   faithful choice independent of which side of that mismatch eventually
//!   moves.
//! - Python's `code_nav_items` never raises (`except Exception` swallows any
//!   failure from `code_sections` and falls back to the full `_SECTION_SPECS`
//!   skeleton), so this port makes it infallible (`-> Vec<(String, String)>`,
//!   no `Result`) rather than matching `server.rs`'s speculative `?` on it.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{bail, Result};
use serde_json::{json, Map, Value};

use crate::model::{get_code_bundle, CodeBundle};
use crate::run_state::{is_running, read_run_state, run_progress};
use crate::state::read_json;

// WIRING: none of these three modules exist yet under v2/kl4a-rs/codekb/src/.
#[allow(unused_imports)]
use crate::context::code_context;
#[allow(unused_imports)]
use crate::pipeline::{
    configured_mining, configured_repo_root, rebuild_risk, start_code_bundle_build, StartBuildOptions,
};
#[allow(unused_imports)]
use crate::render::CodeRenderer;

/// A GET query string, already split into repeated-key form: mirrors Python's
/// `query: dict[str, list[str]]` throughout `web.py`.
pub type Query = HashMap<String, Vec<String>>;

// ---------------------------------------------------------------------------
// String utilities -- html.escape / urllib.parse.quote / urlencode
// equivalents. Stdlib semantics, not app-specific behavior, so these are not
// flagged UNCONFIRMED.
// ---------------------------------------------------------------------------

/// Mirrors bare `escape(...)` calls (`from html import escape`), `quote=True`
/// default: order matters, `&` must go first.
fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// Mirrors `urllib.parse.quote(s)` (default `safe='/'`): percent-encodes
/// every byte outside the unreserved set plus `/`.
fn quote(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Mirrors `urllib.parse.quote_plus`: like `quote` but space -> `+` and `/`
/// is *not* in the safe set. Used by `urlencode`, matching
/// `urllib.parse.urlencode`'s default `quote_via=quote_plus`.
fn quote_plus(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b' ' => out.push('+'),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Mirrors `urllib.parse.urlencode(pairs)`.
fn urlencode(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", quote_plus(k), quote_plus(v)))
        .collect::<Vec<_>>()
        .join("&")
}

// ---------------------------------------------------------------------------
// Query helpers -- `query.get(key, [default])[0] or default` and friends.
// ---------------------------------------------------------------------------

/// Mirrors `query.get(key, [default])[0] or default`: first value for `key`,
/// or `default` if the key is missing, the list is empty, or the first value
/// is the empty string (Python `or` falls through on falsy, not just missing).
fn q1<'a>(q: &'a Query, key: &str, default: &'a str) -> &'a str {
    match q.get(key).and_then(|v| v.first()) {
        Some(s) if !s.is_empty() => s.as_str(),
        _ => default,
    }
}

/// `query.get(key, [""])[0]` without the `or` fallback (used where Python's
/// own code omits the `or ""`, which is a no-op difference since the default
/// is already "").
fn q1_opt<'a>(q: &'a Query, key: &str) -> Option<&'a str> {
    q.get(key).and_then(|v| v.first().map(|s| s.as_str()))
}

/// `query.items()` flattened to `(key, value)` pairs in repeated-field order.
/// Python dicts preserve insertion order; a Rust `HashMap` does not, so a
/// rendered query string here can carry its parameters in a different order
/// than Python would -- functionally identical, cosmetically different.
fn q_pairs(q: &Query) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (k, values) in q.iter() {
        for v in values {
            out.push((k.clone(), v.clone()));
        }
    }
    out
}

fn v_str(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn v_str_or<'a>(v: &'a Value, key: &str, default: &'a str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or(default)
}

fn v_i64(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn v_f64(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

/// Python truthiness on a `dict.get(...)` result.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None => false,
        Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

fn join_str_list(v: Option<&Value>, sep: &str) -> String {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|x| x.as_str().unwrap_or("").to_string())
                .collect::<Vec<_>>()
                .join(sep)
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Nav / dispatch data (py: lines 67-172)
// ---------------------------------------------------------------------------

/// Mirrors `_ARCH_VIEWS`: `(mode, label, knowledge_section_key)`.
const ARCH_VIEWS: &[(&str, &str, &str)] = &[
    ("endpoints", "HTTP endpoints", "endpoints"),
    ("models", "Database models", "data_models"),
    ("schemas", "API schemas", "schemas"),
    ("deps", "External packages", "dependencies"),
    ("programs", "Programs", "cobol_programs"),
    ("copybooks", "Copybooks", "cobol_copybooks"),
    ("fileio", "File I/O", "cobol_file_io"),
    ("calls", "External calls", "cobol_external_calls"),
];

/// Mirrors `_ARCH_MODES = [(mode, label) for mode, label, _ in _ARCH_VIEWS]`.
fn arch_modes() -> Vec<(&'static str, &'static str)> {
    ARCH_VIEWS.iter().map(|(m, l, _)| (*m, *l)).collect()
}

/// Mirrors `_SECTION_SPECS`. The `architecture` entry's third element is
/// `_ARCH_MODES`, computed rather than literal in Python too.
fn section_specs() -> Vec<(&'static str, &'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        (
            "overview",
            "Overview",
            vec![("summary", "Summary"), ("pipeline", "Build")],
        ),
        (
            "code",
            "Code",
            vec![
                ("files", "Files"),
                ("modules", "Modules"),
                ("symbols", "Symbols"),
                ("relations", "Relations"),
                ("graph", "Graph"),
            ],
        ),
        ("architecture", "Architecture", arch_modes()),
        ("knowledge", "Knowledge", vec![]),
        ("docs", "Docs", vec![]),
        ("agent", "Agent", vec![]),
    ]
}

/// Mirrors `_UNLISTED_SECTIONS`.
fn unlisted_sections() -> Vec<(&'static str, &'static str, Vec<(&'static str, &'static str)>)> {
    vec![("search", "Search", vec![])]
}

/// Mirrors `_DETAIL_MODES`.
fn detail_modes() -> HashSet<(&'static str, &'static str)> {
    [("code", "symbol"), ("search", "")].into_iter().collect()
}

/// Mirrors `LEGACY_CODE_ROUTES`.
fn legacy_code_routes() -> HashMap<&'static str, (&'static str, &'static str)> {
    [
        ("pipeline", ("overview", "pipeline")),
        ("sources", ("code", "files")),
        ("modules", ("code", "modules")),
        ("symbols", ("code", "symbols")),
        ("symbol", ("code", "symbol")),
        ("relations", ("code", "relations")),
        ("graph", ("code", "graph")),
        ("endpoints", ("architecture", "endpoints")),
        ("models", ("architecture", "models")),
        ("schemas", ("architecture", "schemas")),
        ("deps", ("architecture", "deps")),
        ("review", ("knowledge", "")),
        ("reports", ("docs", "")),
    ]
    .into_iter()
    .collect()
}

/// Mirrors `CODE_VIEWS = [route for route, _, _ in _SECTION_SPECS]`. Not
/// referenced by any other ported function in this file (Python keeps it as
/// a module-level export, presumably for an external consumer such as a CLI
/// argument validator); ported anyway since it is a real top-level symbol.
#[allow(dead_code)]
pub fn code_views() -> Vec<&'static str> {
    section_specs().into_iter().map(|(route, _, _)| route).collect()
}

/// Mirrors `_SEARCHABLE`: the two set-literals unioned.
fn searchable() -> HashSet<(&'static str, &'static str)> {
    let mut s: HashSet<(&'static str, &'static str)> = [
        ("code", "files"),
        ("code", "modules"),
        ("code", "symbols"),
        ("code", "relations"),
        ("code", "graph"),
    ]
    .into_iter()
    .collect();
    for (mode, _, _) in ARCH_VIEWS {
        if *mode != "deps" {
            s.insert(("architecture", mode));
        }
    }
    s
}

/// Mirrors `_PER_PAGE = 50`.
const PER_PAGE: usize = 50;

/// Mirrors `_GRAPH_MODES`.
const GRAPH_MODES: &[(&str, &str)] = &[
    ("symbol", "Symbol neighborhood"),
    ("package", "Package structure"),
    ("layered", "Request flow (layered)"),
];

/// Mirrors `_RUN_STATUS_TEXT`.
fn run_status_text(status: &str) -> &str {
    match status {
        "queued" => "Queued",
        "running" => "Running",
        "succeeded" => "Completed",
        "failed" => "Failed",
        other => other,
    }
}

/// Mirrors `_STAGE_MARK`.
fn stage_mark(status: &str) -> &str {
    match status {
        "pending" => "\u{b7}",
        "running" => "\u{25b6}",
        "done" => "\u{2713}",
        "failed" => "\u{2717}",
        "skipped" => "\u{2013}",
        _ => "?",
    }
}

/// Mirrors `_CLAIM_FILTERS`.
const CLAIM_FILTERS: &[&str] = &["all", "review_required", "proposed", "anchored", "unanchored", "retired"];

/// Mirrors `REVIEW_ACTIONS`.
fn review_actions(action: &str) -> Option<&'static str> {
    match action {
        "approve" => Some("approved"),
        "reject" => Some("rejected"),
        "defer" => Some("deferred"),
        _ => None,
    }
}

/// Mirrors `_DOCS_RAIL`.
const DOCS_RAIL: &[(&str, &str)] = &[
    ("overview.md", "Overview"),
    ("request-flow.md", "Request flow"),
    ("index.md", "Home"),
    ("code/index.md", "Code"),
    ("code/modules/index.md", "Modules"),
    ("code/symbols/index.md", "Symbols"),
    ("code/endpoints/index.md", "Endpoints"),
    ("code/data-models/index.md", "Data models"),
    ("sources/index.md", "Sources"),
];

/// Mirrors `CODE_EXTRA_STYLE`, byte-for-byte.
const CODE_EXTRA_STYLE: &str = "<style>
.exact{color:var(--ok);} .inferred{color:var(--warn);} .unresolved{color:var(--bad);}
.tier4{color:var(--section);font-weight:600;}
.qname{font-family:ui-monospace,SFMono-Regular,Consolas,monospace;}
.count{color:var(--muted);font-weight:400;font-size:13px;}
.searchbar{display:flex;gap:8px;flex-wrap:wrap;margin-bottom:14px;}
.searchbar input[type=text]{flex:1;min-width:220px;} .searchbar select{width:auto;}
.kv{display:grid;grid-template-columns:150px 1fr;gap:6px 16px;margin:0;}
.kv dt{color:var(--muted);} .kv dd{margin:0;overflow-wrap:anywhere;}
.bar{height:8px;border-radius:4px;background:var(--accent);display:inline-block;vertical-align:middle;}
.stat span{color:var(--muted);font-size:12px;text-transform:uppercase;letter-spacing:.03em;}
.segmented.filters a{display:flex;gap:7px;align-items:center;}
.segmented.filters .rail-count{float:none;}
.browser-bar{display:flex;gap:12px;align-items:center;flex-wrap:wrap;margin:0 0 16px;}
.browser-bar .segmented{margin:0;}
.browser-search{flex:1;min-width:260px;margin:0;}
.rail-group{margin:12px 0 6px;color:var(--muted);font-size:12px;text-transform:uppercase;letter-spacing:.03em;}
.rail-group:first-child{margin-top:0;}
.claim-rail{max-height:calc(100vh - 330px);}
.claim-row{display:grid;grid-template-columns:auto minmax(0,1fr);gap:3px 9px;padding:8px 10px;
  border:1px solid var(--line);border-radius:6px;background:#fff;align-items:start;}
.claim-row.active{border-color:var(--accent);box-shadow:inset 3px 0 0 var(--accent);}
.claim-row input[type=checkbox]{grid-row:1;margin-top:2px;}
.claim-row a{grid-column:2;color:var(--ink);text-decoration:none;overflow-wrap:anywhere;}
.claim-row a:hover{text-decoration:underline;}
.claim-meta{grid-column:2;color:var(--muted);font-size:12px;display:flex;gap:6px;
  align-items:center;flex-wrap:wrap;}
.claim-group{margin:12px 0 6px;}
.hit{display:flex;gap:10px;align-items:baseline;justify-content:space-between;padding:7px 0;
  border-bottom:1px solid var(--line);flex-wrap:wrap;}
.hit:last-child{border-bottom:0;}
.hit a{color:var(--ink);text-decoration:none;overflow-wrap:anywhere;}
.hit a:hover{text-decoration:underline;}
.hit-meta{display:flex;gap:6px;align-items:center;font-size:12px;flex-shrink:0;}
.crumbs{display:flex;gap:8px;align-items:center;flex-wrap:wrap;margin:0 0 12px;font-size:13px;}
.crumbs a{color:var(--accent);text-decoration:none;} .crumbs a:hover{text-decoration:underline;}
.crumbs span{color:var(--muted);}
.pager{display:flex;gap:12px;align-items:center;justify-content:space-between;flex-wrap:wrap;margin:0 0 12px;}
.pager-controls{display:flex;gap:8px;}
.batch-bar{border:1px solid var(--accent);border-radius:8px;padding:12px;margin-top:10px;background:#f4f9fb;
  display:grid;gap:9px;}
</style>";

/// Mirrors `_REVIEW_BATCH_SCRIPT`, byte-for-byte.
const REVIEW_BATCH_SCRIPT: &str = r#"<script>
(function () {
  function init() {
    var form = document.getElementById("review-batch");
    if (!form) { return; }
    var boxes = function () { return form.querySelectorAll('input[name="item_ids"]'); };
    var count = document.getElementById("review-batch-count");

    function refresh() {
      var n = form.querySelectorAll('input[name="item_ids"]:checked').length;
      if (count) { count.textContent = n; }
      form.querySelectorAll("button[data-batch]").forEach(function (b) { b.disabled = n === 0; });
      var bar = document.getElementById("batch-bar");
      if (bar) { bar.classList.toggle("hidden", n === 0); }
    }
    form.addEventListener("change", function (event) {
      var scope = event.target.getAttribute("data-select-all");
      if (scope !== null) {
        var on = event.target.checked;
        var within = scope ? form.querySelectorAll('input[data-group="' + scope + '"]') : boxes();
        within.forEach(function (b) { b.checked = on; });
      }
      refresh();
    });
    refresh();
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
</script>"#;

/// Mirrors `_MERMAID_SNIPPET`, byte-for-byte.
const MERMAID_SNIPPET: &str = r#"<script type="module">import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';mermaid.initialize({ startOnLoad: true, securityLevel: 'strict' });</script>"#;

/// Mirrors `_DOCS_STYLE`, byte-for-byte.
const DOCS_STYLE: &str = "<style>.docbody h1{font-size:22px;} .docbody h2{font-size:18px;margin-top:18px;}.docbody h3{font-size:15px;margin-top:14px;} .docbody pre.mermaid{background:transparent;text-align:center;}.docbody details.fm{margin:0 0 12px;} .docbody details.fm pre{white-space:pre-wrap;}.docbody code{font-family:ui-monospace,SFMono-Regular,Consolas,monospace;}.docbody table{border-collapse:collapse;} .docbody td,.docbody th{padding:4px 10px;border-bottom:1px solid var(--border,#ddd);}</style>";

// ---------------------------------------------------------------------------
// Architecture-populated check (py: `_mode_is_populated`, lines 250-272)
// ---------------------------------------------------------------------------

/// Whether one `_ARCH_VIEWS` knowledge-section key has data. `model.rs`
/// already promotes four of the eight keys (`endpoints`, `data_models`,
/// `schemas`, `dependencies`) to top-level `Vec<Value>` fields on
/// `CodeBundle` rather than leaving them nested under `cb.arch`, so those
/// four are read from the promoted field; the four COBOL-only keys
/// (`cobol_programs`, `cobol_copybooks`, `cobol_file_io`,
/// `cobol_external_calls`) are not promoted and are read from `cb.arch`
/// directly, exactly like Python's `cb.arch.get(key)`. The *behavior* this
/// answers (is this arch view populated) is unchanged either way.
fn arch_truthy(cb: &CodeBundle, key: &str) -> bool {
    match key {
        "endpoints" => !cb.endpoints.is_empty(),
        "data_models" => !cb.data_models.is_empty(),
        "schemas" => !cb.schemas.is_empty(),
        "dependencies" => !cb.dependencies.is_empty(),
        _ => cb
            .arch
            .get(key)
            .map(|v| truthy(Some(v)))
            .unwrap_or(false),
    }
}

/// Mirrors `_is_built`.
fn is_built(cb: &CodeBundle) -> bool {
    !cb.symbols.is_empty() || !cb.sources.is_empty()
}

/// Mirrors `_mode_is_populated`.
fn mode_is_populated(cb: &CodeBundle, section: &str, mode: &str) -> bool {
    if section == "architecture" {
        let key = ARCH_VIEWS
            .iter()
            .find(|(m, _, _)| *m == mode)
            .map(|(_, _, k)| *k)
            .unwrap_or("");
        return arch_truthy(cb, key);
    }
    if section == "code" {
        return match mode {
            "files" => !cb.sources.is_empty(),
            "modules" => !cb.modules.is_empty(),
            "symbols" => !cb.symbols.is_empty(),
            "relations" => !cb.relations.is_empty(),
            "graph" => !cb.symbols.is_empty(),
            _ => true,
        };
    }
    true
}

/// Mirrors `_section_is_populated`.
fn section_is_populated(cb: &CodeBundle, bundle_dir: &Path, section: &str) -> bool {
    if section == "knowledge" {
        return !cb.items.is_empty();
    }
    if section == "docs" {
        let reports = bundle_dir.join("reports");
        let any_doc = DOCS_RAIL
            .iter()
            .any(|(entry, _)| bundle_dir.join(entry).exists());
        let any_report = reports.is_dir()
            && std::fs::read_dir(&reports)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .any(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
                })
                .unwrap_or(false);
        return any_doc || any_report;
    }
    if section == "code" {
        return !cb.sources.is_empty() || !cb.symbols.is_empty();
    }
    true
}

// ---------------------------------------------------------------------------
// Route resolution + dispatch (py: `code_sections` .. `render_code_page`,
// lines 289-522)
// ---------------------------------------------------------------------------

/// Mirrors `code_sections`.
pub fn code_sections(
    bundle_dir: &Path,
) -> Result<Vec<(String, String, Vec<(String, String)>)>> {
    let cb = get_code_bundle(bundle_dir)?;
    if !is_built(&cb) {
        return Ok(section_specs()
            .into_iter()
            .map(|(route, label, modes)| {
                let live = if route == "architecture" { Vec::new() } else { modes };
                (
                    route.to_string(),
                    label.to_string(),
                    live.into_iter()
                        .map(|(m, l)| (m.to_string(), l.to_string()))
                        .collect(),
                )
            })
            .collect());
    }
    let mut sections = Vec::new();
    for (route, label, modes) in section_specs() {
        let mut live: Vec<(&str, &str)> = modes
            .iter()
            .copied()
            .filter(|(m, _)| mode_is_populated(&cb, route, m))
            .collect();
        if route == "overview" {
            live = modes.clone();
        } else if (!modes.is_empty() && live.is_empty())
            || (modes.is_empty() && !section_is_populated(&cb, bundle_dir, route))
        {
            continue;
        }
        sections.push((
            route.to_string(),
            label.to_string(),
            live.into_iter()
                .map(|(m, l)| (m.to_string(), l.to_string()))
                .collect(),
        ));
    }
    Ok(sections)
}

/// Mirrors `code_nav_items`. Infallible: Python's own `except Exception`
/// swallows any failure from `code_sections` and falls back to the full
/// `_SECTION_SPECS` skeleton ("An unreadable bundle must still render a nav,
/// e.g. on an error page.").
pub fn code_nav_items(bundle_dir: &Path) -> Vec<(String, String)> {
    match code_sections(bundle_dir) {
        Ok(sections) => sections.into_iter().map(|(r, l, _)| (r, l)).collect(),
        Err(_) => section_specs()
            .into_iter()
            .map(|(r, l, _)| (r.to_string(), l.to_string()))
            .collect(),
    }
}

/// Mirrors `resolve_code_route`.
pub fn resolve_code_route(bundle_dir: &Path, view: &str, query: &Query) -> (String, String) {
    let trimmed = view.trim_matches('/');
    let mut section = if trimmed.is_empty() { "overview" } else { trimmed }.to_string();
    if unlisted_sections().iter().any(|(r, _, _)| *r == section) {
        return (section, String::new());
    }
    let specs = section_specs();
    if !specs.iter().any(|(r, _, _)| *r == section) {
        if let Some(&(s, m)) = legacy_code_routes().get(section.as_str()) {
            return (s.to_string(), m.to_string());
        }
        section = "overview".to_string();
    }
    let modes: Vec<(&str, &str)> = specs
        .iter()
        .find(|(r, _, _)| *r == section)
        .map(|(_, _, m)| m.clone())
        .unwrap_or_default();
    if modes.is_empty() {
        return (section, String::new());
    }
    let requested = q1(query, "mode", "").trim().to_string();
    let mut valid: HashSet<&str> = modes.iter().map(|(m, _)| *m).collect();
    for (s, m) in detail_modes() {
        if s == section {
            valid.insert(m);
        }
    }
    if valid.contains(requested.as_str()) {
        return (section, requested);
    }
    // Default to a mode this bundle can actually fill (see docstring in the
    // Python source: architecture views are contributed per language, so the
    // first mode in the spec is the first Python one, which would land a
    // COBOL bundle on an empty page).
    let live: Vec<(String, String)> = code_sections(bundle_dir)
        .ok()
        .and_then(|secs| secs.into_iter().find(|(r, _, _)| r == &section).map(|(_, _, m)| m))
        .unwrap_or_default();
    let first_mode = live
        .first()
        .map(|(m, _)| m.clone())
        .unwrap_or_else(|| modes[0].0.to_string());
    (section, first_mode)
}

/// Mirrors `legacy_code_redirect`.
pub fn legacy_code_redirect(view: &str, query: &Query) -> Option<String> {
    let trimmed = view.trim_matches('/');
    let &(section, mode) = legacy_code_routes().get(trimmed)?;
    let mut params: Vec<(String, String)> =
        q_pairs(query).into_iter().filter(|(k, _)| k != "mode").collect();
    if trimmed == "review" {
        // The review queue is now a filter on the claim list, not a page.
        params.insert(0, ("filter".to_string(), "review_required".to_string()));
    } else if trimmed == "reports" {
        // A report is Markdown at a path, which is what the docs view takes.
        let name = query
            .get("name")
            .and_then(|vs| vs.iter().find(|v| !v.is_empty()))
            .cloned()
            .unwrap_or_default();
        params.retain(|(k, _)| k != "name");
        if !name.is_empty() {
            params.insert(0, ("path".to_string(), format!("reports/{name}")));
        }
    }
    if !mode.is_empty() {
        params.insert(0, ("mode".to_string(), mode.to_string()));
    }
    if params.is_empty() {
        Some(format!("/{section}"))
    } else {
        Some(format!("/{section}?{}", urlencode(&params)))
    }
}

/// Mirrors `code_active_view`.
pub fn code_active_view(view: &str) -> String {
    let route = view.trim_matches('/');
    if unlisted_sections().iter().any(|(r, _, _)| *r == route) {
        return route.to_string();
    }
    if section_specs().iter().any(|(r, _, _)| *r == route) {
        return route.to_string();
    }
    legacy_code_routes()
        .get(route)
        .map(|(s, _)| s.to_string())
        .unwrap_or_else(|| "overview".to_string())
}

/// Mirrors `render_code_page`.
pub fn render_code_page(bundle_dir: &Path, view: &str, query: &Query) -> Result<String> {
    let cb = get_code_bundle(bundle_dir)?;
    let (section, mode) = resolve_code_route(bundle_dir, view, query);
    let bar = if detail_modes().contains(&(section.as_str(), mode.as_str())) {
        String::new()
    } else {
        browser_bar(bundle_dir, &cb, &section, &mode, query)
    };
    let body = dispatch_renderer(&section, &mode, &cb, query, bundle_dir)?;
    Ok(format!("{CODE_EXTRA_STYLE}{bar}{body}"))
}

/// Mirrors `_RENDERERS`: the (section, mode) -> renderer dispatch table,
/// ported as a lookup-and-call rather than a hand-enumerated subset. The
/// `_ ->` fallback mirrors `_RENDERERS.get((section, mode)) or
/// _RENDERERS[("overview", "summary")]` in `render_code_page`: an
/// unrecognised pair (which `resolve_code_route` should never actually
/// produce) falls back to the overview summary renderer.
///
/// Returns `Result` uniformly across every branch, even though only two of
/// the nineteen Python renderers (`_pipeline`, and `_graph`'s package/layered
/// modes) can actually raise -- both call into `.pipeline`/`.render`
/// unguarded, propagating like every other unhandled Python exception up to
/// the HTTP layer's broad catch. The other seventeen are pure `Value`
/// rendering and cannot fail; they are wrapped in `Ok(...)` here so every
/// branch has the one shared signature, matching how Python's `_RENDERERS`
/// dict holds one uniform `(cb, q, d) -> str` shape regardless of which
/// renderers happen to be fallible underneath.
fn dispatch_renderer(
    section: &str,
    mode: &str,
    cb: &CodeBundle,
    query: &Query,
    bundle_dir: &Path,
) -> Result<String> {
    match (section, mode) {
        ("overview", "summary") => Ok(overview(cb, query, bundle_dir)),
        ("overview", "pipeline") => pipeline_view(cb, query, bundle_dir),
        ("code", "files") => Ok(sources_view(cb, query, bundle_dir)),
        ("code", "modules") => Ok(modules_view(cb, query, bundle_dir)),
        ("code", "symbols") => Ok(symbols_view(cb, query, bundle_dir)),
        ("code", "symbol") => Ok(symbol_view(cb, query, bundle_dir)),
        ("code", "relations") => Ok(relations_view(cb, query, bundle_dir)),
        ("code", "graph") => graph_view(cb, query, bundle_dir),
        ("architecture", "endpoints") => Ok(endpoints_view(cb, query, bundle_dir)),
        ("architecture", "models") => Ok(models_view(cb, query, bundle_dir)),
        ("architecture", "schemas") => Ok(schemas_view(cb, query, bundle_dir)),
        ("architecture", "deps") => Ok(deps_view(cb, query, bundle_dir)),
        ("architecture", "programs") => Ok(programs_view(cb, query, bundle_dir)),
        ("architecture", "copybooks") => Ok(copybooks_view(cb, query, bundle_dir)),
        ("architecture", "fileio") => Ok(file_io_view(cb, query, bundle_dir)),
        ("architecture", "calls") => Ok(external_calls_view(cb, query, bundle_dir)),
        ("knowledge", "") => Ok(knowledge_view(cb, query, bundle_dir)),
        ("docs", "") => Ok(docs_view(cb, query, bundle_dir)),
        ("agent", "") => Ok(agent_view(cb, query, bundle_dir)),
        ("search", "") => Ok(search_view(cb, query, bundle_dir)),
        _ => Ok(overview(cb, query, bundle_dir)),
    }
}

// ---------------------------------------------------------------------------
// Small shared renderers (py: lines 178-214)
// ---------------------------------------------------------------------------

/// Mirrors `_object_cell`.
fn object_cell(cb: &CodeBundle, relation: &Value, context: &str) -> String {
    let target = relation.get("object").and_then(Value::as_str).unwrap_or("");
    if cb.symbol_by_id.contains_key(target) {
        link_symbol(cb, target, context)
    } else {
        format!("<span class=\"qname muted\">{}</span>", escape(target))
    }
}

/// Mirrors `_in_repo_badge`.
fn in_repo_badge(in_repo: Option<&Value>) -> &'static str {
    if truthy(in_repo) {
        "<span class=\"status ok\">in repo</span>"
    } else {
        "<span class=\"status warn\">not in repo</span>"
    }
}

/// Mirrors `_rstatus`.
fn rstatus(status: &str) -> String {
    let e = escape(status);
    format!("<span class=\"status {e}\">{e}</span>")
}

/// Mirrors `_link_symbol`.
fn link_symbol(cb: &CodeBundle, sid: &str, context: &str) -> String {
    if let Some(&idx) = cb.symbol_by_id.get(sid) {
        let qname = v_str(&cb.symbols[idx], "qualified_name");
        format!(
            "<a href=\"/code?mode=symbol&id={}{}\" class=\"qname\">{}</a>",
            quote(sid),
            context,
            escape(&qname)
        )
    } else {
        format!("<span class=\"qname muted\">{}</span>", escape(sid))
    }
}

/// Mirrors `_list_context`: the filters of the list being rendered, as a
/// query fragment to carry.
fn list_context(query: &Query) -> String {
    let allowed: HashSet<&str> = ["q", "kind", "predicate", "status", "graph"].into_iter().collect();
    let keep: Vec<(String, String)> = q_pairs(query)
        .into_iter()
        .filter(|(k, v)| allowed.contains(k.as_str()) && !v.is_empty())
        .collect();
    if keep.is_empty() {
        String::new()
    } else {
        format!("&{}", urlencode(&keep))
    }
}

// ---------------------------------------------------------------------------
// Mode switch / filters / browser bar (py: lines 395-499)
// ---------------------------------------------------------------------------

/// Mirrors `_mode_switch`: the section's mode switch. The search term follows
/// you across modes, and a selected symbol follows you between the
/// symbol-shaped modes; `id` is not carried into Files or Modules, where it
/// would name a different kind of record.
fn mode_switch(bundle_dir: &Path, section: &str, mode: &str, query: &Query) -> String {
    let modes: Vec<(String, String)> = code_sections(bundle_dir)
        .ok()
        .and_then(|secs| secs.into_iter().find(|(r, _, _)| r == section).map(|(_, _, m)| m))
        .unwrap_or_default();
    if modes.len() < 2 {
        return String::new();
    }
    let symbol_scoped: HashSet<&str> = ["symbols", "symbol", "graph"].into_iter().collect();
    let term = query
        .get("q")
        .and_then(|vs| vs.iter().find(|v| !v.is_empty()))
        .cloned()
        .unwrap_or_default();
    let selected = query
        .get("id")
        .and_then(|vs| vs.iter().find(|v| !v.is_empty()))
        .cloned()
        .unwrap_or_default();
    let mut links = String::new();
    for (name, label) in &modes {
        let mut params: Vec<(String, String)> = vec![("mode".to_string(), name.clone())];
        if !term.is_empty() {
            params.push(("q".to_string(), term.clone()));
        }
        if !selected.is_empty() && symbol_scoped.contains(mode) && symbol_scoped.contains(name.as_str()) {
            params.push(("id".to_string(), selected.clone()));
        }
        let css = if name == mode { "active" } else { "" };
        links.push_str(&format!(
            "<a class=\"{css}\" href=\"/{section}?{}\">{}</a>",
            urlencode(&params),
            escape(label)
        ));
    }
    format!("<nav class=\"segmented\">{links}</nav>")
}

/// Mirrors `_select`.
fn select(name: &str, current: &str, values: &[String], any_label: &str) -> String {
    let mut opts = String::new();
    let mut all = vec![String::new()];
    all.extend(values.iter().cloned());
    for v in &all {
        let sel = if v == current { " selected" } else { "" };
        let ev = escape(v);
        let label = if ev.is_empty() { any_label.to_string() } else { ev.clone() };
        opts.push_str(&format!("<option value=\"{ev}\"{sel}>{label}</option>"));
    }
    format!("<select name=\"{name}\">{opts}</select>")
}

/// Mirrors `_filter_extras`: mode-specific filters, rendered into the shared
/// search bar.
fn filter_extras(cb: &CodeBundle, section: &str, mode: &str, query: &Query) -> String {
    let get = |key: &str| q1_opt(query, key).unwrap_or("").to_string();
    match (section, mode) {
        ("code", "symbols") => {
            let mut kinds: Vec<String> = cb.symbols.iter().map(|s| v_str(s, "kind")).collect();
            kinds.sort();
            kinds.dedup();
            select("kind", &get("kind"), &kinds, "any kind")
        }
        ("code", "relations") => {
            let mut predicates: Vec<String> = cb
                .rel_summary
                .get("by_predicate")
                .and_then(Value::as_object)
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            predicates.sort();
            let mut statuses: Vec<String> = cb
                .rel_summary
                .get("by_resolution_status")
                .and_then(Value::as_object)
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            statuses.sort();
            format!(
                "{}{}",
                select("predicate", &get("predicate"), &predicates, "any predicate"),
                select("status", &get("status"), &statuses, "any status")
            )
        }
        ("architecture", "endpoints") => {
            let mut methods: Vec<String> = cb.endpoints.iter().map(|e| v_str(e, "method")).collect();
            methods.sort();
            methods.dedup();
            select("method", &get("method"), &methods, "any method")
        }
        ("code", "graph") => {
            // Keep the graph's own view selection when searching.
            let g = get("graph");
            let g = if g.is_empty() { "symbol".to_string() } else { g };
            format!("<input type=\"hidden\" name=\"graph\" value=\"{}\">", escape(&g))
        }
        _ => String::new(),
    }
}

/// Mirrors `_browser_bar`: mode switch and search box as one control strip.
fn browser_bar(bundle_dir: &Path, cb: &CodeBundle, section: &str, mode: &str, query: &Query) -> String {
    let switch = mode_switch(bundle_dir, section, mode, query);
    if !searchable().contains(&(section, mode)) {
        return switch;
    }
    let term = q1(query, "q", "");
    let form = format!(
        "<form class=\"searchbar browser-search\" method=\"get\"><input type=\"hidden\" name=\"mode\" value=\"{}\"><input type=\"text\" name=\"q\" value=\"{}\" placeholder=\"search this view\">{}<button>Search</button></form>",
        escape(mode),
        escape(term),
        filter_extras(cb, section, mode, query)
    );
    format!("<div class=\"browser-bar\">{switch}{form}</div>")
}

// ---------------------------------------------------------------------------
// Next action / pagination / active claims (py: lines 528-629)
// ---------------------------------------------------------------------------

/// Mirrors `_next_action`: what this bundle needs next, as one line with the
/// control to do it.
fn next_action(cb: &CodeBundle, run_state: &Value, progress: &Value) -> String {
    if truthy(progress.get("active")) {
        let current = progress.get("current_label").and_then(Value::as_str).unwrap_or("");
        let current = if current.is_empty() { "starting" } else { current };
        let completed = v_i64(progress, "completed");
        let total = v_i64(progress, "total");
        return format!(
            "<div class=\"notice\"><strong>Build running</strong> &mdash; {} ({}/{} stages). <a href=\"/overview?mode=pipeline\">Watch progress</a>. This page refreshes every 3 seconds.</div>",
            escape(current), completed, total
        );
    }
    let status = run_state.get("status").and_then(Value::as_str).unwrap_or("");
    if status == "failed" {
        let err = run_state.get("error").and_then(Value::as_str).unwrap_or("");
        let err = if err.is_empty() { "unknown error" } else { err };
        return format!(
            "<div class=\"notice\"><strong>The last build failed</strong> &mdash; {}. <a href=\"/overview?mode=pipeline\">Review and re-run</a>.</div>",
            escape(err)
        );
    }
    if cb.symbols.is_empty() {
        return "<div class=\"notice\"><strong>Nothing built yet.</strong> Point this bundle at a repository folder and run the pipeline to populate it. <a href=\"/overview?mode=pipeline\">Run the pipeline</a>.</div>".to_string();
    }
    let pending = cb
        .items
        .iter()
        .filter(|i| {
            let lifecycle = i.get("lifecycle_status").and_then(Value::as_str).unwrap_or("active");
            let lifecycle = if lifecycle.is_empty() { "active" } else { lifecycle };
            let review_required = truthy(i.get("review_required"))
                || i.get("knowledge_tier").and_then(Value::as_i64) == Some(4);
            let proposed = i.get("review_status").and_then(Value::as_str) == Some("proposed");
            lifecycle == "active" && review_required && proposed
        })
        .count();
    if pending > 0 {
        return format!(
            "<div class=\"notice\"><strong>{pending} claim(s) awaiting review.</strong> An unreviewed Tier 4 claim is excluded from <span class=\"qname\">code.context</span>, so enrichment that is never reviewed is paid for and then not used. <a href=\"/knowledge?filter=review_required\">Review them</a>.</div>"
        );
    }
    "<div class=\"notice\"><strong>This bundle is built and reviewed.</strong> <a href=\"/agent\">Try a retrieval</a> to see what an agent gets back.</div>".to_string()
}

/// Mirrors `_page_slice`: one page of a list, plus the control to reach the
/// rest. Generic over the item type so it serves every list view (symbols,
/// relations, claims, ranked graph symbols, ...) exactly like the Python
/// version, which takes a plain `list`.
fn page_slice<'a, T>(items: &'a [T], query: &Query, per_page: usize) -> (&'a [T], String) {
    let total = items.len();
    let pages = std::cmp::max(1, (total + per_page - 1) / per_page);
    let raw = q1(query, "page", "1").trim().to_string();
    let digits = raw.trim_start_matches('-');
    let is_digit = !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit());
    let mut page: i64 = if is_digit { raw.parse().unwrap_or(1) } else { 1 };
    page = page.max(1).min(pages as i64);
    let page = page as usize;
    let start = (page - 1) * per_page;
    let end = std::cmp::min(start + per_page, total);
    let shown = &items[start.min(total)..end];
    (shown, pager(query, page, pages, total, start, per_page))
}

/// Mirrors `_pager`.
fn pager(query: &Query, page: usize, pages: usize, total: usize, start: usize, per_page: usize) -> String {
    if total == 0 {
        return String::new();
    }
    let upto = std::cmp::min(start + per_page, total);
    if pages == 1 {
        return format!("<p class=\"muted\">{total} result(s).</p>");
    }
    let href = |target: i64| -> String {
        let mut params: Vec<(String, String)> =
            q_pairs(query).into_iter().filter(|(k, _)| k != "page").collect();
        params.push(("page".to_string(), target.to_string()));
        format!("?{}", urlencode(&params))
    };
    let prev = if page > 1 {
        format!(
            "<a class=\"button-link secondary\" href=\"{}\">Previous</a>",
            href(page as i64 - 1)
        )
    } else {
        "<span class=\"muted\">Previous</span>".to_string()
    };
    let nxt = if page < pages {
        format!(
            "<a class=\"button-link secondary\" href=\"{}\">Next</a>",
            href(page as i64 + 1)
        )
    } else {
        "<span class=\"muted\">Next</span>".to_string()
    };
    format!(
        "<div class=\"pager\"><span class=\"muted\">Showing {}\u{2013}{} of {} \u{b7} page {} of {}</span><span class=\"pager-controls\">{prev}{nxt}</span></div>",
        start + 1,
        upto,
        total,
        page,
        pages
    )
}

/// Mirrors `_active_claims`: the claims that describe the code as it stands
/// now.
fn active_claims(items: &[Value]) -> Vec<&Value> {
    items
        .iter()
        .filter(|i| {
            let s = i.get("lifecycle_status").and_then(Value::as_str).unwrap_or("active");
            let s = if s.is_empty() { "active" } else { s };
            s == "active"
        })
        .collect()
}

/// Renders a JSON scalar the way an f-string would stringify it.
fn value_display(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Null => "None".to_string(),
        Value::Bool(b) => if *b { "True".to_string() } else { "False".to_string() },
        other => other.to_string(),
    }
}

/// Python `x or "-"` on a possibly-empty string.
fn or_dash(s: &str) -> &str {
    if s.is_empty() { "-" } else { s }
}

fn m_str<'a>(m: &'a Map<String, Value>, key: &str) -> &'a str {
    m.get(key).and_then(Value::as_str).unwrap_or("")
}

// ---------------------------------------------------------------------------
// Overview / pipeline (py: lines 632-953)
// ---------------------------------------------------------------------------

/// Mirrors `_overview`.
fn overview(cb: &CodeBundle, _query: &Query, bundle_dir: &Path) -> String {
    let r = &cb.repository;
    // Python iterates `dict.items()` in insertion order; `cb.detected_languages`
    // is a `serde_json::Map` (a `BTreeMap` -- see `state.rs`'s note on
    // `preserve_order` not being enabled), so this iterates key-sorted
    // instead. Same content, cosmetically different order.
    let lang_parts: Vec<String> = cb
        .detected_languages
        .iter()
        .map(|(k, v)| format!("{k} ({})", value_display(v)))
        .collect();
    let langs = if lang_parts.is_empty() { "n/a".to_string() } else { lang_parts.join(", ") };
    let active = active_claims(&cb.items);

    let mut stats: Vec<(usize, String, String)> = vec![
        (cb.sources.len(), "sources".to_string(), "/code?mode=files".to_string()),
        (cb.modules.len(), "modules".to_string(), "/code?mode=modules".to_string()),
        (cb.symbols.len(), "symbols".to_string(), "/code?mode=symbols".to_string()),
        (cb.relations.len(), "relations".to_string(), "/code?mode=relations".to_string()),
        (active.len(), "claims".to_string(), "/knowledge?filter=all".to_string()),
    ];
    // The claims tile opens /knowledge, which lists active claims, so it
    // counts active claims: a headline that disagrees with the page it opens
    // reads as a lost record. Retired claims are shown as their own tile,
    // only when there are some, pointing at the filter that lists them.
    if cb.items.len() > active.len() {
        stats.push((
            cb.items.len() - active.len(),
            "retired claims".to_string(),
            "/knowledge?filter=retired".to_string(),
        ));
    }
    let stat_html: String = stats
        .iter()
        .map(|(n, l, href)| {
            let value = if *n > 0 { format!("<a href=\"{href}\">{n}</a>") } else { n.to_string() };
            format!("<div class=\"stat\"><span>{l}</span><strong>{value}</strong></div>")
        })
        .collect();

    let mut res_entries: Vec<(String, i64)> = cb
        .rel_summary
        .get("by_resolution_status")
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(0))).collect())
        .unwrap_or_default();
    res_entries.sort_by(|a, b| b.1.cmp(&a.1));
    let res_total: i64 = {
        let s: i64 = res_entries.iter().map(|(_, v)| v).sum();
        if s == 0 { 1 } else { s }
    };
    let res_rows: String = res_entries
        .iter()
        .map(|(k, v)| {
            format!(
                "<tr><td>{}</td><td>{v}</td><td><span class=\"bar\" style=\"width:{}px\"></span></td></tr>",
                rstatus(k),
                300 * v / res_total
            )
        })
        .collect();

    let mut pred_entries: Vec<(String, i64)> = cb
        .rel_summary
        .get("by_predicate")
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(0))).collect())
        .unwrap_or_default();
    pred_entries.sort_by(|a, b| b.1.cmp(&a.1));
    let pmax: i64 = pred_entries.iter().map(|(_, v)| *v).max().unwrap_or(1).max(1);
    let pred_rows: String = pred_entries
        .iter()
        .map(|(k, v)| {
            format!(
                "<tr><td><a href=\"/code?mode=relations&predicate={}\">{}</a></td><td>{v}</td><td><span class=\"bar\" style=\"width:{}px\"></span></td></tr>",
                quote(k),
                escape(k),
                260 * v / pmax
            )
        })
        .collect();

    let mut tier_entries: Vec<(String, i64)> = cb
        .know_summary
        .get("by_tier")
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(0))).collect())
        .unwrap_or_default();
    tier_entries.sort_by(|a, b| a.0.cmp(&b.0));
    let tier_rows: String = tier_entries
        .iter()
        .map(|(k, v)| {
            let review_tag = if k == "4" { " <span class=\"tag tier4\">review</span>" } else { "" };
            format!(
                "<tr><td><a href=\"/knowledge?mode=claims&tier={}\">Tier {}</a>{review_tag}</td><td>{v}</td></tr>",
                escape(k),
                escape(k)
            )
        })
        .collect();

    let arch_panel = if truthy(Some(&cb.arch)) {
        let s = cb.arch.get("summary").cloned().unwrap_or(Value::Object(Map::new()));
        let fw: Vec<String> = cb
            .frameworks
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        let fw_joined = if fw.is_empty() { "n/a".to_string() } else { fw.join(", ") };
        let arch_cell = |key: &str, label: &str, href: &str| -> String {
            let count = s.get(key).and_then(Value::as_i64).unwrap_or(0);
            let value = if count != 0 {
                format!("<a href=\"/architecture?mode={href}\">{count}</a>")
            } else {
                count.to_string()
            };
            format!("<div class=\"stat\"><span>{label}</span><strong>{value}</strong></div>")
        };
        let arch_cells: String = [
            ("endpoints", "endpoints", "endpoints"),
            ("routers", "routers", "endpoints"),
            ("data_models", "models", "models"),
            ("schemas", "schemas", "schemas"),
            ("dependencies", "dependencies", "deps"),
        ]
        .iter()
        .map(|(key, l, href)| arch_cell(key, l, href))
        .collect();
        format!(
            "<div class=\"panel\"><h3>Architecture <span class=\"count\">{}</span></h3><div class=\"stats\">{arch_cells}</div></div>",
            escape(&fw_joined)
        )
    } else {
        "<div class=\"panel\"><h3>Architecture</h3><p class=\"muted\">Not detected yet. Re-run the pipeline with <strong>Detect architecture</strong> enabled to extract endpoints, models, schemas, and dependencies.</p><div class=\"toolbar\"><a class=\"button-link secondary\" href=\"/overview?mode=pipeline\">Go to Build</a></div></div>".to_string()
    };

    let run_state = read_run_state(bundle_dir);
    let progress = run_progress(&run_state);
    let refresh = if truthy(progress.get("active")) {
        "<meta http-equiv=\"refresh\" content=\"3\">"
    } else {
        ""
    };

    let path = escape(or_dash(m_str(r, "path")));
    let vcs = escape(or_dash(m_str(r, "vcs")));
    let branch = escape(or_dash(m_str(r, "branch")));
    let commit_full = m_str(r, "commit");
    let commit_12: String = commit_full.chars().take(12).collect();
    let commit = escape(or_dash(&commit_12));

    format!(
        "{refresh}<h2>Overview</h2>\n{}\n<div class=\"stats\">{stat_html}</div>\n{arch_panel}\n<div class=\"panel\"><h3>Repository</h3><dl class=\"kv\">\n<dt>Path</dt><dd class=\"qname\">{path}</dd>\n<dt>VCS / Branch</dt><dd class=\"qname\">{vcs} \u{b7} {branch}</dd>\n<dt>Commit</dt><dd class=\"qname\">{commit}</dd>\n<dt>Languages</dt><dd>{}</dd></dl></div>\n<div class=\"panel\"><h3>Relation resolution</h3><div class=\"table-wrap\"><table class=\"code-table\">{res_rows}</table></div>\n<p class=\"muted\">Unresolved = calls to symbols outside the parsed repo. Inferred = heuristic match. Exact = parser-resolved.</p></div>\n<div class=\"grid\">\n<div class=\"panel\"><h3>Relations by predicate</h3><div class=\"table-wrap\"><table class=\"code-table\">{pred_rows}</table></div></div>\n<div class=\"panel\"><h3>Knowledge by tier</h3><div class=\"table-wrap\"><table class=\"code-table\">{tier_rows}</table></div></div></div>",
        next_action(cb, &run_state, &progress),
        escape(&langs)
    )
}

/// Mirrors `_run_panel`: render the current or most recent run. Shown above
/// the form so the answer to "is it doing anything" is the first thing on
/// the page.
fn run_panel(state: &Value, progress: &Value) -> String {
    if !truthy(Some(state)) {
        return "<div class=\"panel\"><h3>Last run</h3><p class=\"muted\">No pipeline run recorded for this bundle yet.</p></div>".to_string();
    }
    let status = progress.get("status").and_then(Value::as_str).unwrap_or("");
    let heading = run_status_text(status);
    let rows: String = state
        .get("stages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|stage| {
            let st_status = v_str_or(stage, "status", "");
            let muted = if matches!(st_status, "pending" | "skipped") { "muted" } else { "" };
            let label = v_str(stage, "label");
            let detail = stage.get("detail").and_then(Value::as_str).unwrap_or("");
            format!(
                "<tr><td style=\"width:2em\">{}</td><td class=\"{muted}\">{}</td><td class=\"muted\">{}</td></tr>",
                stage_mark(st_status),
                escape(&label),
                escape(detail)
            )
        })
        .collect();
    let mut banner = String::new();
    if truthy(progress.get("active")) {
        let current = progress.get("current_label").and_then(Value::as_str).unwrap_or("");
        let current = if current.is_empty() { "starting" } else { current };
        banner = format!(
            "<p><strong>{}</strong> \u{2014} {} ({}/{} stages). This page refreshes every 3 seconds.</p>",
            escape(heading),
            escape(current),
            v_i64(progress, "completed"),
            v_i64(progress, "total")
        );
    } else if status == "failed" {
        let err = state.get("error").and_then(Value::as_str).unwrap_or("");
        let err = if err.is_empty() { "unknown error" } else { err };
        banner = format!(
            "<p><strong>{}</strong> \u{2014} {}</p><p class=\"muted\">Earlier stages that completed have already been written to the bundle.</p>",
            escape(heading),
            escape(err)
        );
    } else if status == "succeeded" {
        let s = state.get("summary").cloned().unwrap_or(Value::Object(Map::new()));
        banner = format!(
            "<p><strong>{}</strong> \u{2014} {} symbol(s), {} relation(s), {} claim(s), {} validation error(s).</p>",
            escape(heading),
            v_i64(&s, "symbols"),
            v_i64(&s, "relations"),
            v_i64(&s, "knowledge_items"),
            v_i64(&s, "errors")
        );
    }
    let mining_mode = or_dash(state.get("mining_mode").and_then(Value::as_str).unwrap_or(""));
    let provider = or_dash(state.get("provider").and_then(Value::as_str).unwrap_or(""));
    let started_at = or_dash(state.get("started_at").and_then(Value::as_str).unwrap_or(""));
    let meta = format!(
        "<p class=\"muted\">Mode: {} \u{b7} provider: {} \u{b7} started: {}</p>",
        escape(mining_mode),
        escape(provider),
        escape(started_at)
    );
    let bar = if v_i64(progress, "total") != 0 {
        format!(
            "<div style=\"background:#eef2f5;border-radius:6px;height:8px;margin:8px 0\"><div style=\"width:{}%;background:var(--accent);height:8px;border-radius:6px\"></div></div>",
            v_i64(progress, "percent")
        )
    } else {
        String::new()
    };
    format!(
        "<div class=\"panel\"><h3>Pipeline run</h3>{banner}{bar}<div class=\"table-wrap\"><table class=\"code-table\">{rows}</table></div>{meta}</div>"
    )
}

/// Mirrors `_pipeline`: run the code pipeline against a repository folder
/// path. This is where the static/hybrid mining choice is made, so the
/// option sits next to the repo path it applies to rather than being a
/// CLI-only flag.
///
/// WIRING: `configured_mining`, `configured_repo_root` and `rebuild_risk`
/// (from `.pipeline`, not yet ported) and `llm_settings::{is_configured,
/// missing_fields}` (from the top-level Python `kl4a` package, no Rust
/// counterpart crate/module exists yet) are called unguarded here exactly as
/// Python calls them unguarded -- an error propagates via `?`, matching an
/// uncaught Python exception bubbling to the HTTP layer's broad catch.
fn pipeline_view(cb: &CodeBundle, _query: &Query, bundle_dir: &Path) -> Result<String> {
    let provider_status = if crate::llm_settings::is_configured() {
        "LLM provider <span class=\"status ok\">configured</span>.".to_string()
    } else {
        format!(
            "LLM provider <span class=\"status warn\">not configured</span> \u{2014} missing {}. A hybrid run will still produce a valid static bundle, but enrichment will be skipped. Configure it with the sopkb settings page or the provider env vars.",
            escape(&crate::llm_settings::missing_fields().join(", "))
        )
    };

    let mining = configured_mining(bundle_dir);
    let mode = {
        let m = mining.get("mode").and_then(Value::as_str).unwrap_or("");
        if m.is_empty() { "static".to_string() } else { m.to_string() }
    };
    let selection = mining.get("selection").cloned().unwrap_or(Value::Object(Map::new()));
    let repo_path = {
        let recorded = m_str(&cb.repository, "path");
        if recorded.is_empty() { configured_repo_root(bundle_dir) } else { recorded.to_string() }
    };
    // Falls back to the manifest because a freshly created bundle has not
    // been scanned yet, and the scan is what writes the repository record.
    let enrichment = read_json(&cb.state_dir.join("code_knowledge.json"), Value::Object(Map::new()))
        .get("enrichment")
        .cloned()
        .unwrap_or(Value::Object(Map::new()));

    let hybrid_selected = if mode == "hybrid" || mode == "llm" { " selected" } else { "" };
    let static_selected = if hybrid_selected.is_empty() { " selected" } else { "" };

    let run_state = read_run_state(bundle_dir);
    let progress = run_progress(&run_state);
    let run_panel_html = run_panel(&run_state, &progress);
    // Only poll while something is actually happening, so an idle page is
    // static.
    let active = truthy(progress.get("active"));
    let refresh = if active { "<meta http-equiv=\"refresh\" content=\"3\">" } else { "" };
    let disabled = if active { " disabled" } else { "" };

    let risk = rebuild_risk(bundle_dir, &repo_path)?;
    let confirm_html = if truthy(risk.get("confirm_required")) {
        format!(
            "<div class=\"panel\" style=\"border-color:var(--warn)\"><p><strong>This bundle was built from a different repository.</strong></p><p class=\"qname\">built from: {}</p><p class=\"qname\">about to use: {}</p><p>Rebuilding retires all {} existing claim(s) and replaces them with claims from the new repository. Review decisions on the retired claims are kept, but they no longer describe this bundle.</p><label class=\"choice\"><input type=\"checkbox\" name=\"confirm_bundle_update\"{disabled}> Yes, rebuild this bundle from the new repository</label></div>",
            escape(v_str_or(&risk, "recorded_repo", "")),
            escape(&repo_path),
            v_i64(&risk, "active_claims")
        )
    } else if v_str_or(&risk, "reason", "") == "same_repo" {
        format!(
            "<p class=\"muted\">Updating in place from the same repository. {} existing claim(s) will be re-derived; review decisions are preserved.</p>",
            v_i64(&risk, "active_claims")
        )
    } else {
        "<p class=\"muted\">First run for this bundle. Nothing to overwrite.</p>".to_string()
    };

    let mut last_run = String::new();
    if truthy(enrichment.get("attempted")) {
        let rejected: Vec<Value> = enrichment
            .get("rejected")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let reasons: String = rejected
            .iter()
            .take(20)
            .map(|entry| {
                format!(
                    "<tr><td class='qname'>{}</td><td>{}</td></tr>",
                    escape(v_str_or(entry, "symbol_id", "")),
                    escape(v_str_or(entry, "reason", ""))
                )
            })
            .collect();
        let more = if rejected.len() > 20 {
            format!("<p class='muted'>{} more not shown.</p>", rejected.len() - 20)
        } else {
            String::new()
        };
        let cache = enrichment.get("cache").cloned().unwrap_or(Value::Object(Map::new()));
        let cache_enabled = truthy(cache.get("enabled"));
        let cache_stats = if cache_enabled {
            format!(
                "<div class=\"stat\"><span>reused from cache</span><strong>{}</strong></div><div class=\"stat\"><span>provider calls</span><strong>{}</strong></div>",
                v_i64(&cache, "hits"),
                v_i64(&enrichment, "provider_calls")
            )
        } else {
            String::new()
        };
        let cache_note = if cache_enabled {
            "Unchanged symbols are answered from <span class='qname'>.codekb/cache/</span> rather than re-sent. Editing a file, or changing the model or prompt, re-asks only what changed. Clear it with <span class='qname'>sopkb code cache clear &lt;bundle_dir&gt;</span>.".to_string()
        } else {
            "Response caching is disabled for this bundle, so every run re-sends every selected symbol.".to_string()
        };
        let reasons_table = if !rejected.is_empty() {
            format!(
                "<div class=\"table-wrap\"><table class=\"code-table\"><tr><th>Symbol</th><th>Rejection reason</th></tr>{reasons}</table></div>{more}"
            )
        } else {
            String::new()
        };
        last_run = format!(
            "<div class=\"panel\"><h3>Last LLM enrichment</h3>\n<div class=\"stats\">\n<div class=\"stat\"><span>symbols attempted</span><strong>{}</strong></div>\n<div class=\"stat\"><span>claims accepted</span><strong>{}</strong></div>\n<div class=\"stat\"><span>claims rejected</span><strong>{}</strong></div>\n{cache_stats}\n</div>\n<p class=\"muted\">{cache_note}</p>\n{reasons_table}\n</div>",
            v_i64(&enrichment, "attempted"),
            v_i64(&enrichment, "accepted"),
            rejected.len()
        );
    }

    let min_relations = selection.get("min_relations").and_then(Value::as_i64).unwrap_or(2);
    let max_symbols = selection.get("max_symbols").and_then(Value::as_i64).unwrap_or(250);

    Ok(format!(
        "{refresh}<h2>Pipeline</h2>\n{run_panel_html}\n<section class=\"panel\">\n  <form class=\"inline-form\" method=\"post\" action=\"/code-pipeline\">\n    <label>Repository folder path<input name=\"repo_dir\" value=\"{}\" placeholder=\"D:\\path\\to\\repo\"{disabled}></label>\n    <div class=\"option-grid\">\n      <div class=\"option-box\"><label>Mining mode<select name=\"mining_mode\"{disabled}>\n        <option value=\"static\"{static_selected}>static (deterministic, offline)</option>\n        <option value=\"hybrid\"{hybrid_selected}>hybrid (static + LLM enrichment)</option>\n      </select></label></div>\n      <div class=\"option-box\"><label class=\"choice\"><input type=\"checkbox\" name=\"architecture\" checked{disabled}> Detect architecture</label></div>\n      <div class=\"option-box\"><label class=\"choice\"><input type=\"checkbox\" name=\"render\" checked{disabled}> Render human layer</label></div>\n    </div>\n    {confirm_html}\n    <div class=\"toolbar\">\n      <button type=\"submit\"{disabled}>Run Pipeline</button>\n    </div>\n  </form>\n  <p class=\"muted\">Current bundle mode: <strong>{}</strong>. The mode is stored in\n  <span class=\"qname\">manifest.yaml</span> under <span class=\"qname\">codekb.mining</span>, so CLI runs use it too.\n  The run continues on the server if you close this page.</p>\n</section>\n<div class=\"panel\"><h3>How the modes differ</h3>\n<p><strong>static</strong> runs AST parsing, relation resolution, and rule-based claim extraction. Reproducible,\noffline, no network calls. Use this for gate evidence and CI.</p>\n<p><strong>hybrid</strong> runs all of the above first, then asks an LLM to interpret the highest-signal symbols.\nThe LLM may only cite symbol and relation ids that static extraction already produced, and must quote code\nverbatim from the symbol's own lines. Claims citing invented ids are dropped; claims whose quote cannot be\nlocated are kept but demoted to Tier 4. The static baseline is never replaced, so a provider failure degrades\nto a valid static bundle.</p>\n<p class=\"muted\">{provider_status} Hybrid sends repository source to the configured endpoint. Selection gate:\n{min_relations}+ relations, max\n{max_symbols} symbols.</p>\n</div>\n{last_run}",
        escape(&repo_path),
        escape(&mode)
    ))
}

// ---------------------------------------------------------------------------
// Code views: sources / modules / symbols / symbol detail (py: lines 956-1126)
// ---------------------------------------------------------------------------

/// Mirrors `_sources`.
fn sources_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    let sel_query = q1(query, "id", "").to_string();
    let q = q1(query, "q", "").to_lowercase();
    let mut srcs: Vec<&Value> = cb
        .sources
        .iter()
        .filter(|s| q.is_empty() || v_str(s, "path").to_lowercase().contains(&q))
        .collect();
    srcs.sort_by(|a, b| v_str(a, "path").cmp(&v_str(b, "path")));
    let rail: String = srcs
        .iter()
        .map(|s| {
            let sid = v_str(s, "id");
            let path = v_str(s, "path");
            let count = cb.symbols_by_source.get(&path).map(|v| v.len()).unwrap_or(0);
            let active = if sid == sel_query { "active" } else { "" };
            format!(
                "<a class=\"{active}\" href=\"/code?mode=files&id={}\"><span class=\"rail-count\">{count}</span>{}</a>",
                quote(&sid),
                escape(&path)
            )
        })
        .collect();
    let sel = if sel_query.is_empty() {
        srcs.first().map(|s| v_str(s, "id")).unwrap_or_default()
    } else {
        sel_query
    };
    let src = cb.sources.iter().find(|s| v_str(s, "id") == sel);
    let detail = match src {
        None => "<div class=\"panel\">No source selected.</div>".to_string(),
        Some(src) => {
            let path = v_str(src, "path");
            let mut syms: Vec<&Value> = cb
                .symbols_by_source
                .get(&path)
                .map(|idxs| idxs.iter().map(|&i| &cb.symbols[i]).collect())
                .unwrap_or_default();
            syms.sort_by_key(|s| v_i64(s, "line_start"));
            let sym_rows: String = syms
                .iter()
                .map(|s| {
                    format!(
                        "<tr><td>{}</td><td><span class=\"tag\">{}</span></td><td class=\"muted\">{}-{}</td></tr>",
                        link_symbol(cb, &v_str(s, "id"), ""),
                        escape(&v_str(s, "kind")),
                        v_i64(s, "line_start"),
                        v_i64(s, "line_end")
                    )
                })
                .collect();
            let checksum: String = v_str(src, "checksum").chars().take(24).collect();
            format!(
                "<div class=\"panel\"><div class=\"detail-header\"><h3 class=\"qname\">{}</h3>\n<span class=\"status {}\">{}</span></div>\n<dl class=\"kv\"><dt>Language</dt><dd>{}</dd><dt>Kind</dt><dd>{}</dd>\n<dt>Checksum</dt><dd class=\"qname muted\">{}\u{2026}</dd></dl>\n<h3 style=\"margin-top:14px\">Symbols <span class=\"count\">{}</span></h3>\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Symbol</th><th>Kind</th><th>Lines</th></tr></thead>\n<tbody>{}</tbody></table></div></div>",
                escape(&path),
                escape(&v_str(src, "parse_status")),
                escape(&v_str(src, "parse_status")),
                escape(&v_str(src, "language")),
                escape(&v_str(src, "kind")),
                escape(&checksum),
                syms.len(),
                if sym_rows.is_empty() {
                    "<tr><td colspan=3 class=\"muted\">no parsed symbols</td></tr>".to_string()
                } else {
                    sym_rows
                }
            )
        }
    };
    format!(
        "<h2>Sources <span class=\"count\">{}</span></h2>\n<div class=\"split\"><div class=\"rail\">{rail}</div>{detail}</div>",
        srcs.len()
    )
}

/// Mirrors `_modules`.
fn modules_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    let q = q1(query, "q", "").to_lowercase();
    let mut mods: Vec<&Value> = cb
        .modules
        .iter()
        .filter(|m| {
            q.is_empty()
                || v_str(m, "qualified_name").to_lowercase().contains(&q)
                || v_str(m, "file").to_lowercase().contains(&q)
        })
        .collect();
    mods.sort_by(|a, b| v_str(a, "qualified_name").cmp(&v_str(b, "qualified_name")));
    let mut sym_count: HashMap<String, i64> = HashMap::new();
    for s in &cb.symbols {
        let mid = s.get("module_id").and_then(Value::as_str).unwrap_or("").to_string();
        *sym_count.entry(mid).or_insert(0) += 1;
    }
    let total = mods.len();
    let (shown, pager_html) = page_slice(&mods, query, PER_PAGE);
    let rows: String = shown
        .iter()
        .map(|m| {
            let id = v_str(m, "id");
            let imports = m.get("imports").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0);
            format!(
                "<tr><td class=\"qname\">{}</td><td class=\"qname muted\">{}</td><td>{imports}</td><td>{}</td></tr>",
                escape(&v_str(m, "qualified_name")),
                escape(&v_str(m, "file")),
                sym_count.get(&id).copied().unwrap_or(0)
            )
        })
        .collect();
    format!(
        "<h2>Modules <span class=\"count\">{total}</span></h2>{pager_html}\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Module</th><th>File</th><th>Imports</th><th>Symbols</th></tr></thead><tbody>{rows}</tbody></table></div>"
    )
}

/// Mirrors `_symbols`.
fn symbols_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    let q = q1(query, "q", "").to_lowercase();
    let kind = q1(query, "kind", "");
    let mut syms: Vec<&Value> = cb.symbols.iter().collect();
    if !q.is_empty() {
        syms.retain(|s| {
            v_str(s, "qualified_name").to_lowercase().contains(&q) || v_str(s, "id").to_lowercase().contains(&q)
        });
    }
    if !kind.is_empty() {
        syms.retain(|s| v_str(s, "kind") == kind);
    }
    syms.sort_by(|a, b| v_str(a, "qualified_name").cmp(&v_str(b, "qualified_name")));
    let total = syms.len();
    let (shown, pager_html) = page_slice(&syms, query, PER_PAGE);
    let ctx = list_context(query);
    let rows: String = shown
        .iter()
        .map(|s| {
            let sid = v_str(s, "id");
            let out_n = cb.out_rel.get(&sid).map(|v| v.len()).unwrap_or(0);
            let in_n = cb.in_rel.get(&sid).map(|v| v.len()).unwrap_or(0);
            format!(
                "<tr><td>{}</td><td><span class=\"tag\">{}</span></td><td class=\"qname muted\">{}:{}</td><td>{out_n}/{in_n}</td></tr>",
                link_symbol(cb, &sid, &ctx),
                escape(&v_str(s, "kind")),
                escape(&v_str(s, "file")),
                v_i64(s, "line_start")
            )
        })
        .collect();
    format!(
        "<h2>Symbols <span class=\"count\">{total}</span></h2>{pager_html}\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Symbol</th><th>Kind</th><th>Location</th><th title=\"out/in relations\">out/in</th></tr></thead><tbody>{rows}</tbody></table></div>"
    )
}

/// Mirrors `_breadcrumb`: where you are, and the way back to exactly where
/// you came from.
fn breadcrumb(
    query: &Query,
    section: &str,
    section_label: &str,
    mode: &str,
    mode_label: &str,
    leaf: &str,
) -> String {
    let allowed: HashSet<&str> = ["q", "kind", "predicate", "status", "graph"].into_iter().collect();
    let mut keep: Vec<(String, String)> = q_pairs(query)
        .into_iter()
        .filter(|(k, v)| allowed.contains(k.as_str()) && !v.is_empty())
        .collect();
    keep.insert(0, ("mode".to_string(), mode.to_string()));
    let back = format!("/{section}?{}", urlencode(&keep));
    let terms: String = keep
        .iter()
        .filter(|(k, _)| k != "mode")
        .map(|(k, v)| format!("{k}:\"{}\"", escape(v)))
        .collect::<Vec<_>>()
        .join(" ");
    let hint = if terms.is_empty() { String::new() } else { format!(" <span class=\"muted\">{terms}</span>") };
    format!(
        "<nav class=\"crumbs\"><a href=\"/{section}\">{}</a><span>/</span><a href=\"{back}\">{}</a>{hint}<span>/</span><span class=\"qname\">{}</span></nav>",
        escape(section_label),
        escape(mode_label),
        escape(leaf)
    )
}

/// Mirrors `_symbol`.
fn symbol_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    let sid = q1(query, "id", "").to_string();
    let Some(&s_idx) = cb.symbol_by_id.get(&sid) else {
        return "<h2>Symbol not found</h2>".to_string();
    };
    let s = &cb.symbols[s_idx];
    let evidence_id = v_str(s, "evidence_id");
    let excerpt = cb
        .evidence_by_id
        .get(&evidence_id)
        .map(|&idx| {
            let truncated: String = v_str(&cb.evidence[idx], "excerpt").chars().take(2500).collect();
            escape(&truncated)
        })
        .unwrap_or_else(|| "(no evidence)".to_string());

    let out_idxs = cb.out_rel.get(&sid).cloned().unwrap_or_default();
    let in_idxs = cb.in_rel.get(&sid).cloned().unwrap_or_default();
    let mut out_rels: Vec<&Value> = out_idxs.iter().map(|&i| &cb.relations[i]).collect();
    let mut in_rels: Vec<&Value> = in_idxs.iter().map(|&i| &cb.relations[i]).collect();
    out_rels.sort_by(|a, b| v_str(a, "predicate").cmp(&v_str(b, "predicate")));
    in_rels.sort_by(|a, b| v_str(a, "predicate").cmp(&v_str(b, "predicate")));

    let rel_row = |r: &Value, direction: &str| -> String {
        let other = if direction == "out" { v_str(r, "object") } else { v_str(r, "subject") };
        let other_html = if cb.symbol_by_id.contains_key(&other) {
            link_symbol(cb, &other, "")
        } else {
            format!("<span class=\"qname muted\">{}</span>", escape(&other))
        };
        let relation = r.get("relation").cloned().unwrap_or(Value::Object(Map::new()));
        let st = v_str(&relation, "resolution_status");
        let conf = relation.get("confidence").map(value_display).unwrap_or_default();
        format!(
            "<tr><td><span class=\"tag\">{}</span></td><td>{other_html}</td><td>{}</td><td class=\"muted\">{conf}</td></tr>",
            escape(&v_str(r, "predicate")),
            rstatus(&st)
        )
    };
    let out_rows: String = out_rels.iter().map(|r| rel_row(r, "out")).collect();
    let in_rows: String = in_rels.iter().map(|r| rel_row(r, "in")).collect();

    let claim_idxs = cb.items_by_symbol.get(&sid).cloned().unwrap_or_default();
    let claims: Vec<&Value> = claim_idxs.iter().map(|&i| &cb.items[i]).collect();
    let claim_rows: String = claims
        .iter()
        .map(|c| {
            let truncated: String = v_str(c, "claim").chars().take(400).collect();
            let tier = c.get("knowledge_tier").and_then(Value::as_i64).unwrap_or(0);
            let tier4 = if tier == 4 { " tier4" } else { "" };
            format!(
                "<tr><td>{}</td><td><span class=\"tag{tier4}\">T{tier}</span></td><td>{}</td></tr>",
                escape(&truncated),
                rstatus(&v_str(c, "review_status"))
            )
        })
        .collect();

    let doc = v_str(s, "docstring");
    let came_from = if query.get("graph").is_some() { "graph" } else { "symbols" };
    let mode_label = if came_from == "graph" { "Graph" } else { "Symbols" };
    let qname = v_str(s, "qualified_name");
    let crumbs = breadcrumb(query, "code", "Code", came_from, mode_label, &qname);
    let signature: String = v_str(s, "signature").chars().take(1500).collect();

    format!(
        "{crumbs}<div class=\"detail-header\"><h2 class=\"qname\">{}</h2>\n<a class=\"button-link secondary\" href=\"/code?mode=graph&graph=symbol&id={}\">View call graph</a></div>\n<div class=\"panel\"><dl class=\"kv\">\n<dt>Kind</dt><dd><span class=\"tag\">{}</span></dd>\n<dt>Module</dt><dd class=\"qname\">{}</dd>\n<dt>Location</dt><dd class=\"qname\">{}:{}-{}</dd>\n<dt>Symbol id</dt><dd class=\"qname muted\">{}</dd></dl></div>\n{}\n<div class=\"panel\"><h3>Signature</h3><pre>{}</pre></div>\n<div class=\"panel\"><h3>Evidence <span class=\"count\">{}</span></h3><pre>{excerpt}</pre></div>\n<div class=\"grid\">\n<div class=\"panel\"><h3>Outgoing (callees) <span class=\"count\">{}</span></h3><div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Predicate</th><th>Target</th><th>Status</th><th>Conf</th></tr></thead><tbody>{}</tbody></table></div></div>\n<div class=\"panel\"><h3>Incoming (callers) <span class=\"count\">{}</span></h3><div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Predicate</th><th>Source</th><th>Status</th><th>Conf</th></tr></thead><tbody>{}</tbody></table></div></div></div>\n<div class=\"panel\"><h3>Knowledge claims <span class=\"count\">{}</span></h3><div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Claim</th><th>Tier</th><th>Review</th></tr></thead><tbody>{}</tbody></table></div></div>",
        escape(&qname),
        quote(&sid),
        escape(&v_str(s, "kind")),
        escape(&v_str(s, "module")),
        escape(&v_str(s, "file")),
        v_i64(s, "line_start"),
        v_i64(s, "line_end"),
        escape(&v_str(s, "id")),
        if doc.is_empty() {
            String::new()
        } else {
            format!("<div class=\"panel\"><h3>Docstring</h3><pre>{}</pre></div>", escape(&doc))
        },
        escape(&signature),
        escape(&v_str(s, "file")),
        out_rels.len(),
        if out_rows.is_empty() { "<tr><td colspan=4 class=muted>none</td></tr>".to_string() } else { out_rows },
        in_rels.len(),
        if in_rows.is_empty() { "<tr><td colspan=4 class=muted>none</td></tr>".to_string() } else { in_rows },
        claims.len(),
        if claim_rows.is_empty() { "<tr><td colspan=3 class=muted>none</td></tr>".to_string() } else { claim_rows },
    )
}

// ---------------------------------------------------------------------------
// Relations / graph (py: lines 1129-1311)
// ---------------------------------------------------------------------------

/// Mirrors `_relations`.
fn relations_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    let predicate = q1(query, "predicate", "");
    let status = q1(query, "status", "");
    let q = q1(query, "q", "").to_lowercase();
    let mut rels: Vec<&Value> = cb.relations.iter().collect();
    if !predicate.is_empty() {
        rels.retain(|r| v_str(r, "predicate") == predicate);
    }
    if !status.is_empty() {
        rels.retain(|r| {
            r.get("relation").and_then(|rr| rr.get("resolution_status")).and_then(Value::as_str) == Some(status)
        });
    }
    if !q.is_empty() {
        rels.retain(|r| v_str(r, "title").to_lowercase().contains(&q) || v_str(r, "object").to_lowercase().contains(&q));
    }
    let total = rels.len();
    let (shown, pager_html) = page_slice(&rels, query, PER_PAGE);
    let ctx = list_context(query);
    let rows: String = shown
        .iter()
        .map(|r| {
            let subject = v_str(r, "subject");
            let subject_html = if cb.symbol_by_id.contains_key(&subject) {
                link_symbol(cb, &subject, &ctx)
            } else {
                escape(&subject)
            };
            let relation = r.get("relation").cloned().unwrap_or(Value::Object(Map::new()));
            format!(
                "<tr><td>{subject_html}</td><td><span class=\"tag\">{}</span></td><td>{}</td><td>{}</td></tr>",
                escape(&v_str(r, "predicate")),
                object_cell(cb, r, &ctx),
                rstatus(&v_str(&relation, "resolution_status"))
            )
        })
        .collect();
    format!(
        "<h2>Relations <span class=\"count\">{total}</span></h2>{pager_html}\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Subject</th><th>Predicate</th><th>Object</th><th>Status</th></tr></thead><tbody>{rows}</tbody></table></div>"
    )
}

/// Mirrors `_mermaid_inner`: strip the fence so the body can go in a
/// `<pre class="mermaid">`.
fn mermaid_inner(fenced: &str) -> String {
    let mut lines: Vec<&str> = fenced.lines().collect();
    if lines.first().map(|l| l.trim().starts_with("```")).unwrap_or(false) {
        lines.remove(0);
    }
    if lines.last().map(|l| l.trim() == "```").unwrap_or(false) {
        lines.pop();
    }
    lines.join("\n")
}

/// Mirrors `_graph`. Three real modes, all ported: symbol neighborhood
/// (delegates to `graph_symbol_body`/`graph_svg`), package structure and
/// request-flow (layered), both of which reuse the render layer's Mermaid
/// builders. WIRING: `CodeRenderer` (from `.render`, not yet ported) is
/// called unguarded, matching Python's own unguarded call.
fn graph_view(cb: &CodeBundle, query: &Query, bundle_dir: &Path) -> Result<String> {
    let mode_raw = q1(query, "graph", "symbol");
    let mode = if GRAPH_MODES.iter().any(|(m, _)| *m == mode_raw) { mode_raw } else { "symbol" };
    let sel = q1(query, "id", "").to_string();
    let pill = |m: &str, label: &str| -> String {
        let href = format!(
            "/code?mode=graph&graph={m}{}",
            if m == "symbol" && !sel.is_empty() { format!("&id={}", quote(&sel)) } else { String::new() }
        );
        let css = if m == mode { "active" } else { "" };
        format!("<a class=\"{css}\" href=\"{href}\">{}</a>", escape(label))
    };
    let tabs: String = GRAPH_MODES.iter().map(|(m, label)| pill(m, label)).collect();
    let header = format!("<h2>Graph</h2><nav class=\"segmented\">{tabs}</nav>");
    if mode == "symbol" {
        return Ok(format!("{header}{}", graph_symbol_body(cb, query)));
    }
    // Aggregate Mermaid views -- reuse the render layer's builders (single
    // source of truth).
    let renderer = CodeRenderer::new(bundle_dir)?;
    let fenced = if mode == "package" {
        renderer.package_tree_mermaid()
    } else {
        renderer.layered_flow_mermaid()
    };
    let inner = mermaid_inner(&fenced);
    if inner.trim().is_empty() {
        return Ok(format!("{header}<div class=\"panel\">Nothing to display for this view yet.</div>"));
    }
    let desc = if mode == "package" {
        "Folder / package hierarchy, annotated with module counts."
    } else {
        "Aggregated route \u{2192} service \u{2192} repository \u{2192} model call flow across all endpoints."
    };
    Ok(format!(
        "{header}<p class=\"muted\">{desc}</p><div class=\"panel\"><pre class=\"mermaid\">{}</pre></div>{MERMAID_SNIPPET}",
        escape(&inner)
    ))
}

/// Mirrors `_graph_symbol_body`.
fn graph_symbol_body(cb: &CodeBundle, query: &Query) -> String {
    let mut sel = q1(query, "id", "").to_string();
    let q = q1(query, "q", "").to_lowercase();
    let mut ranked: Vec<&Value> = cb.symbols.iter().collect();
    ranked.sort_by_key(|s| std::cmp::Reverse(cb.degree.get(&v_str(s, "id")).copied().unwrap_or(0)));
    if !q.is_empty() {
        ranked.retain(|s| v_str(s, "qualified_name").to_lowercase().contains(&q));
    }
    let (ranked_page, pager_html) = page_slice(&ranked, query, PER_PAGE);
    let rail: String = ranked_page
        .iter()
        .map(|s| {
            let sid = v_str(s, "id");
            let active = if sid == sel { "active" } else { "" };
            let degree = cb.degree.get(&sid).copied().unwrap_or(0);
            format!(
                "<a class=\"{active}\" href=\"/code?mode=graph&graph=symbol&id={}\"><span class=\"rail-count\">{degree}</span>{}</a>",
                quote(&sid),
                escape(&v_str(s, "qualified_name"))
            )
        })
        .collect();
    if sel.is_empty() {
        if let Some(first) = ranked.first() {
            sel = v_str(first, "id");
        }
    }
    let detail = if !sel.is_empty() {
        graph_svg(cb, &sel)
    } else {
        "<div class=\"panel\">Pick a symbol.</div>".to_string()
    };
    format!(
        "<p class=\"muted\">Symbol-scoped neighborhood: callers (left) \u{2192} symbol (center) \u{2192} callees (right). Rail ranked by degree.</p>\n<div class=\"graph-layout\"><div><div class=\"rail\">{rail}</div>{pager_html}</div><div>{detail}</div></div>"
    )
}

/// Mirrors `_graph_svg`.
fn graph_svg(cb: &CodeBundle, sid: &str) -> String {
    let Some(&center_idx) = cb.symbol_by_id.get(sid) else {
        return "<div class=\"panel\">Symbol not found.</div>".to_string();
    };
    let center = &cb.symbols[center_idx];
    let out_idxs = cb.out_rel.get(sid).cloned().unwrap_or_default();
    let in_idxs = cb.in_rel.get(sid).cloned().unwrap_or_default();
    let out_rels: Vec<&Value> = out_idxs.iter().map(|&i| &cb.relations[i]).collect();
    let in_rels: Vec<&Value> = in_idxs.iter().map(|&i| &cb.relations[i]).collect();

    let callees: Vec<&Value> = out_rels
        .iter()
        .filter(|r| cb.symbol_by_id.contains_key(&v_str(r, "object")))
        .take(14)
        .copied()
        .collect();
    let callers: Vec<&Value> = in_rels
        .iter()
        .filter(|r| cb.symbol_by_id.contains_key(&v_str(r, "subject")))
        .take(14)
        .copied()
        .collect();
    let ext: Vec<&Value> = out_rels
        .iter()
        .filter(|r| !cb.symbol_by_id.contains_key(&v_str(r, "object")))
        .copied()
        .collect();

    let node_w: i64 = 250;
    let node_h: i64 = 46;
    let row_gap: i64 = 60;
    let (caller_x, center_x, callee_x): (i64, i64, i64) = (150, 480, 810);
    let rows = (callers.len().max(callees.len()).max(1)) as i64;
    let height = (rows * row_gap + 80).max(200);
    let cy = height / 2;

    let short = |sym: &Value| -> String {
        let qname = v_str(sym, "qualified_name");
        let last = qname.rsplit('.').next().unwrap_or("").to_string();
        if last.is_empty() { qname } else { last }
    };
    let node = |x: i64, y: i64, sym: &Value, color: &str, nid: &str| -> String {
        format!(
            "<a href=\"/code?mode=symbol&id={}\"><g class=\"graph-node\"><title>{}</title><rect x=\"{}\" y=\"{}\" width=\"{node_w}\" height=\"{node_h}\" rx=\"7\" fill=\"#fff\" stroke=\"{color}\" stroke-width=\"2\"/><circle cx=\"{}\" cy=\"{y}\" r=\"5\" fill=\"{color}\"/><text x=\"{}\" y=\"{}\">{}</text></g></a>",
            quote(nid),
            escape(&v_str(sym, "qualified_name")),
            x - node_w / 2,
            y - node_h / 2,
            x - node_w / 2 + 16,
            x - node_w / 2 + 30,
            y + 4,
            escape(&short(sym))
        )
    };

    let mut parts = String::new();
    let start_y_callers = if !callers.is_empty() { cy - (callers.len() as i64 - 1) * row_gap / 2 } else { cy };
    for (i, r) in callers.iter().enumerate() {
        let y = start_y_callers + i as i64 * row_gap;
        parts.push_str(&format!(
            "<line x1=\"{}\" y1=\"{y}\" x2=\"{}\" y2=\"{cy}\" stroke=\"#b8c2cc\" stroke-width=\"1.5\"/>",
            caller_x + node_w / 2,
            center_x - node_w / 2
        ));
        parts.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"10\" fill=\"#8a97a5\">{}</text>",
            (caller_x + center_x) / 2 - 20,
            (y + cy) / 2 - 4,
            escape(&v_str(r, "predicate"))
        ));
        let subj = v_str(r, "subject");
        if let Some(&idx) = cb.symbol_by_id.get(&subj) {
            parts.push_str(&node(caller_x, y, &cb.symbols[idx], "var(--section)", &subj));
        }
    }
    let start_y_callees = if !callees.is_empty() { cy - (callees.len() as i64 - 1) * row_gap / 2 } else { cy };
    for (i, r) in callees.iter().enumerate() {
        let y = start_y_callees + i as i64 * row_gap;
        let st = r
            .get("relation")
            .and_then(|rr| rr.get("resolution_status"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let color = match st {
            "exact" => "var(--ok)",
            "inferred" => "var(--warn)",
            _ => "var(--evidence)",
        };
        parts.push_str(&format!(
            "<line x1=\"{}\" y1=\"{cy}\" x2=\"{}\" y2=\"{y}\" stroke=\"#b8c2cc\" stroke-width=\"1.5\"/>",
            center_x + node_w / 2,
            callee_x - node_w / 2
        ));
        parts.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"10\" fill=\"#8a97a5\">{}</text>",
            (center_x + callee_x) / 2 - 20,
            (y + cy) / 2 - 4,
            escape(&v_str(r, "predicate"))
        ));
        let obj = v_str(r, "object");
        if let Some(&idx) = cb.symbol_by_id.get(&obj) {
            parts.push_str(&node(callee_x, y, &cb.symbols[idx], color, &obj));
        }
    }
    parts.push_str(&node(center_x, cy, center, "var(--accent)", sid));
    let svg = format!("<svg width=\"1000\" height=\"{height}\" role=\"img\">{parts}</svg>");
    let ext_note = if !ext.is_empty() {
        format!(
            "<p class=\"muted\">+ {} unresolved/external calls (see <a href=\"/code?mode=symbol&id={}\">symbol page</a>).</p>",
            ext.len(),
            quote(sid)
        )
    } else {
        String::new()
    };
    let legend = "<div class=\"toolbar\"><span class=\"tag\">callers</span><span class=\"status exact\">exact callee</span><span class=\"status inferred\">inferred</span></div>";
    format!(
        "<div class=\"panel\"><div class=\"detail-header\"><h3 class=\"qname\">{}</h3><span class=\"muted\">{} callers \u{b7} {} resolved callees</span></div>{legend}<div class=\"graph-wrap\">{svg}</div>{ext_note}</div>",
        escape(&v_str(center, "qualified_name")),
        callers.len(),
        callees.len()
    )
}

// ---------------------------------------------------------------------------
// Architecture views (py: lines 1314-1541)
// ---------------------------------------------------------------------------

/// Python `str.capitalize()`: first char upper, rest lower.
fn python_capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
    }
}

/// Python `x or "—"` (em dash) fallback.
fn or_emdash(s: &str) -> &str {
    if s.is_empty() { "\u{2014}" } else { s }
}

/// Mirrors `_arch_missing`: an empty state that offers the control, not the
/// command.
fn arch_missing(what: &str) -> String {
    format!(
        "<h2>{}</h2><div class=\"panel\"><p>No {} detected in this bundle.</p><p class=\"muted\">Architecture is extracted during a pipeline run with <strong>Detect architecture</strong> enabled.</p><div class=\"toolbar\"><a class=\"button-link\" href=\"/overview?mode=pipeline\">Go to Build</a></div><p class=\"muted\">From a script: <span class=\"qname\">sopkb code architecture &lt;bundle_dir&gt;</span>.</p></div>",
        escape(&python_capitalize(what)),
        escape(what)
    )
}

/// Mirrors `_endpoints`.
fn endpoints_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    if cb.endpoints.is_empty() {
        return arch_missing("endpoints");
    }
    let q = q1(query, "q", "").to_lowercase();
    let method = q1(query, "method", "");
    let mut eps: Vec<&Value> = cb.endpoints.iter().collect();
    if !q.is_empty() {
        eps.retain(|e| {
            v_str(e, "path").to_lowercase().contains(&q) || v_str(e, "handler_qname").to_lowercase().contains(&q)
        });
    }
    if !method.is_empty() {
        eps.retain(|e| v_str(e, "method") == method);
    }
    let total = eps.len();
    let (shown, pager_html) = page_slice(&eps, query, PER_PAGE);
    let rows: String = shown
        .iter()
        .map(|e| {
            let handler_sid = v_str(e, "handler_symbol_id");
            let handler_html = if !handler_sid.is_empty() { link_symbol(cb, &handler_sid, "") } else { escape(&v_str(e, "handler")) };
            let tags = join_str_list(e.get("tags"), ", ");
            format!(
                "<tr><td><span class=\"tag\">{}</span></td><td class=\"qname\">{}</td><td>{handler_html}</td><td class=\"qname muted\">{}:{}</td><td>{}</td></tr>",
                escape(&v_str(e, "method")),
                escape(&v_str(e, "path")),
                escape(&v_str(e, "file")),
                v_i64(e, "line"),
                escape(&tags)
            )
        })
        .collect();
    let router_rows: String = cb
        .routers
        .iter()
        .map(|r| {
            let mount_prefix = v_str(r, "mount_prefix");
            let mount_prefix = if mount_prefix.is_empty() { "/" } else { &mount_prefix };
            let prefix = v_str(r, "prefix");
            format!(
                "<tr><td class=\"qname\">{}</td><td class=\"qname\">{}</td><td class=\"qname\">{}{}</td><td>{}</td><td>{}</td></tr>",
                escape(&v_str(r, "var")),
                escape(&v_str(r, "module")),
                escape(mount_prefix),
                escape(&prefix),
                if truthy(r.get("mounted")) { "yes" } else { "no" },
                v_i64(r, "endpoint_count")
            )
        })
        .collect();
    format!(
        "<h2>Endpoints <span class=\"count\">{total}</span></h2>\n<p class=\"muted\">FastAPI routes detected from decorators. Path = mount prefix + router prefix + route path.</p>\n{pager_html}\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Method</th><th>Path</th><th>Handler</th><th>Source</th><th>Tags</th></tr></thead><tbody>{rows}</tbody></table></div>\n<h3 style=\"margin-top:18px\">Router wiring <span class=\"count\">{}</span></h3>\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Router</th><th>Module</th><th>Effective prefix</th><th>Mounted</th><th>Endpoints</th></tr></thead><tbody>{router_rows}</tbody></table></div>",
        cb.routers.len()
    )
}

/// Mirrors `_models`.
fn models_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    if cb.data_models.is_empty() {
        return arch_missing("data models");
    }
    let q = q1(query, "q", "").to_lowercase();
    let mut sel = q1(query, "id", "").to_string();
    let models: Vec<&Value> = cb
        .data_models
        .iter()
        .filter(|m| {
            q.is_empty()
                || v_str(m, "qualified_name").to_lowercase().contains(&q)
                || v_str(m, "table").to_lowercase().contains(&q)
        })
        .collect();
    let rail: String = models
        .iter()
        .map(|m| {
            let mid = v_str(m, "id");
            let active = if mid == sel { "active" } else { "" };
            let cols = m.get("columns").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0);
            format!(
                "<a class=\"{active}\" href=\"/models?id={}\"><span class=\"rail-count\">{cols}</span>{}</a>",
                quote(&mid),
                escape(&v_str(m, "name"))
            )
        })
        .collect();
    if sel.is_empty() {
        if let Some(first) = models.first() {
            sel = v_str(first, "id");
        }
    }
    let m = cb.data_models.iter().find(|x| v_str(x, "id") == sel);
    let detail = match m {
        None => "<div class=\"panel\">No model selected.</div>".to_string(),
        Some(m) => {
            let col_rows: String = m
                .get("columns")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|c| {
                    format!(
                        "<tr><td class=\"qname\">{}</td><td>{}</td><td class=\"qname muted\">{}</td></tr>",
                        escape(&v_str(c, "name")),
                        escape(&v_str(c, "kind")),
                        escape(&v_str(c, "type"))
                    )
                })
                .collect();
            let symbol_id = v_str(m, "symbol_id");
            let link = if cb.symbol_by_id.contains_key(&symbol_id) { link_symbol(cb, &symbol_id, "") } else { String::new() };
            let bases = join_str_list(m.get("bases"), ", ");
            let cols_count = m.get("columns").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0);
            let table = v_str(m, "table");
            format!(
                "<div class=\"panel\"><div class=\"detail-header\"><h3 class=\"qname\">{}</h3>\n{link}</div>\n<dl class=\"kv\"><dt>Table</dt><dd class=\"qname\">{}</dd>\n<dt>Bases</dt><dd class=\"qname\">{}</dd>\n<dt>Source</dt><dd class=\"qname\">{}:{}</dd></dl>\n<h3 style=\"margin-top:12px\">Columns <span class=\"count\">{cols_count}</span></h3>\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Field</th><th>Kind</th><th>Type</th></tr></thead>\n<tbody>{}</tbody></table></div></div>",
                escape(&v_str(m, "qualified_name")),
                escape(or_emdash(&table)),
                escape(&bases),
                escape(&v_str(m, "file")),
                v_i64(m, "line"),
                if col_rows.is_empty() { "<tr><td colspan=3 class=\"muted\">none detected</td></tr>".to_string() } else { col_rows }
            )
        }
    };
    format!(
        "<h2>Data models <span class=\"count\">{}</span></h2>\n<p class=\"muted\">SQLAlchemy ORM models (declarative). Fields shown are Column/relationship definitions.</p>\n<div class=\"split\"><div class=\"rail\">{rail}</div>{detail}</div>",
        models.len()
    )
}

/// Mirrors `_schemas`.
fn schemas_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    if cb.schemas.is_empty() {
        return arch_missing("schemas");
    }
    let q = q1(query, "q", "").to_lowercase();
    let mut sel = q1(query, "id", "").to_string();
    let schemas: Vec<&Value> = cb
        .schemas
        .iter()
        .filter(|s| q.is_empty() || v_str(s, "qualified_name").to_lowercase().contains(&q))
        .collect();
    let rail: String = schemas
        .iter()
        .map(|s| {
            let sid = v_str(s, "id");
            let active = if sid == sel { "active" } else { "" };
            let fields = s.get("fields").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0);
            format!(
                "<a class=\"{active}\" href=\"/schemas?id={}\"><span class=\"rail-count\">{fields}</span>{}</a>",
                quote(&sid),
                escape(&v_str(s, "name"))
            )
        })
        .collect();
    if sel.is_empty() {
        if let Some(first) = schemas.first() {
            sel = v_str(first, "id");
        }
    }
    let s = cb.schemas.iter().find(|x| v_str(x, "id") == sel);
    let detail = match s {
        None => "<div class=\"panel\">No schema selected.</div>".to_string(),
        Some(s) => {
            let field_rows: String = s
                .get("fields")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|f| {
                    format!(
                        "<tr><td class=\"qname\">{}</td><td class=\"qname muted\">{}</td></tr>",
                        escape(&v_str(f, "name")),
                        escape(&v_str(f, "type"))
                    )
                })
                .collect();
            let symbol_id = v_str(s, "symbol_id");
            let link = if cb.symbol_by_id.contains_key(&symbol_id) { link_symbol(cb, &symbol_id, "") } else { String::new() };
            let bases = join_str_list(s.get("bases"), ", ");
            let fields_count = s.get("fields").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0);
            format!(
                "<div class=\"panel\"><div class=\"detail-header\"><h3 class=\"qname\">{}</h3>\n{link}</div>\n<dl class=\"kv\"><dt>Bases</dt><dd class=\"qname\">{}</dd>\n<dt>Source</dt><dd class=\"qname\">{}:{}</dd></dl>\n<h3 style=\"margin-top:12px\">Fields <span class=\"count\">{fields_count}</span></h3>\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Field</th><th>Type</th></tr></thead>\n<tbody>{}</tbody></table></div></div>",
                escape(&v_str(s, "qualified_name")),
                escape(&bases),
                escape(&v_str(s, "file")),
                v_i64(s, "line"),
                if field_rows.is_empty() { "<tr><td colspan=2 class=\"muted\">no annotated fields</td></tr>".to_string() } else { field_rows }
            )
        }
    };
    format!(
        "<h2>Schemas <span class=\"count\">{}</span></h2>\n<p class=\"muted\">Pydantic models (request/response schemas), including transitively-derived subclasses.</p>\n<div class=\"split\"><div class=\"rail\">{rail}</div>{detail}</div>",
        schemas.len()
    )
}

/// Mirrors `_deps`.
fn deps_view(cb: &CodeBundle, _query: &Query, _bundle_dir: &Path) -> String {
    if cb.dependencies.is_empty() {
        return arch_missing("external packages");
    }
    let rows: String = cb
        .dependencies
        .iter()
        .map(|d| {
            let modules = d.get("modules").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0);
            format!(
                "<tr><td class=\"qname\">{}</td><td>{}</td><td>{modules}</td></tr>",
                escape(&v_str(d, "package")),
                escape(&v_str(d, "category"))
            )
        })
        .collect();
    format!(
        "<h2>External packages <span class=\"count\">{}</span></h2>\n<p class=\"muted\">External third-party packages imported by the repo (non-stdlib, non-local), by import breadth.</p>\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Package</th><th>Category</th><th>Imported by</th></tr></thead><tbody>{rows}</tbody></table></div>",
        cb.dependencies.len()
    )
}

/// Mirrors `_programs`.
fn programs_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    let progs_all: Vec<&Value> = cb.arch.get("cobol_programs").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    if progs_all.is_empty() {
        return arch_missing("programs");
    }
    let q = q1(query, "q", "").to_lowercase();
    let progs: Vec<&Value> = progs_all
        .into_iter()
        .filter(|p| q.is_empty() || v_str(p, "name").to_lowercase().contains(&q) || v_str(p, "file").to_lowercase().contains(&q))
        .collect();
    let total = progs.len();
    let (shown, pager_html) = page_slice(&progs, query, PER_PAGE);
    let rows: String = shown
        .iter()
        .map(|p| {
            let copybooks = join_str_list(p.get("copybooks"), ", ");
            let calls = join_str_list(p.get("calls"), ", ");
            format!(
                "<tr><td class=\"qname\">{}</td><td class=\"qname muted\">{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(&v_str(p, "name")),
                escape(&v_str(p, "file")),
                v_i64(p, "paragraphs"),
                v_i64(p, "data_items"),
                or_emdash(&escape(&copybooks)),
                or_emdash(&escape(&calls))
            )
        })
        .collect();
    format!(
        "<h2>Programs <span class=\"count\">{total}</span></h2>\n<p class=\"muted\">COBOL programs in this repository, with the copybooks they COPY and the programs they CALL.</p>\n{pager_html}\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Program</th><th>File</th>\n<th>Paragraphs</th><th>Data items</th><th>Copies</th><th>Calls</th></tr></thead><tbody>{rows}</tbody></table></div>"
    )
}

/// Mirrors `_copybooks`.
fn copybooks_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    let books_all: Vec<&Value> = cb.arch.get("cobol_copybooks").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    if books_all.is_empty() {
        return arch_missing("copybooks");
    }
    let q = q1(query, "q", "").to_lowercase();
    let books: Vec<&Value> = books_all.into_iter().filter(|b| q.is_empty() || v_str(b, "name").to_lowercase().contains(&q)).collect();
    let total = books.len();
    let (shown, pager_html) = page_slice(&books, query, PER_PAGE);
    let rows: String = shown
        .iter()
        .map(|b| {
            let file = v_str(b, "file");
            let used_by = b.get("used_by").and_then(Value::as_array).cloned().unwrap_or_default();
            let used_by_joined = join_str_list(b.get("used_by"), ", ");
            format!(
                "<tr><td class=\"qname\">{}</td><td>{}</td><td class=\"qname muted\">{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(&v_str(b, "name")),
                in_repo_badge(b.get("in_repo")),
                if file.is_empty() { "\u{2014}".to_string() } else { escape(&file) },
                v_i64(b, "fields"),
                used_by.len(),
                or_emdash(&escape(&used_by_joined))
            )
        })
        .collect();
    format!(
        "<h2>Copybooks <span class=\"count\">{total}</span></h2>\n<p class=\"muted\">Copybooks and the programs that COPY them. Fan-in is the useful direction: a copybook changed\nwithout checking who copies it is the classic way to break a build. A copybook COPYd but absent from the\nrepository is listed too, rather than hidden.</p>\n{pager_html}\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Copybook</th><th>Present</th><th>File</th>\n<th>Fields</th><th>Used by</th><th>Programs</th></tr></thead><tbody>{rows}</tbody></table></div>"
    )
}

/// Mirrors `_file_io`.
fn file_io_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    let files_all: Vec<&Value> = cb.arch.get("cobol_file_io").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    if files_all.is_empty() {
        return arch_missing("file I/O");
    }
    let q = q1(query, "q", "").to_lowercase();
    let files: Vec<&Value> = files_all.into_iter().filter(|f| q.is_empty() || v_str(f, "name").to_lowercase().contains(&q)).collect();
    let total = files.len();
    let (shown, pager_html) = page_slice(&files, query, PER_PAGE);
    let rows: String = shown
        .iter()
        .map(|f| {
            let reads = join_str_list(f.get("reads"), ", ");
            let writes = join_str_list(f.get("writes"), ", ");
            format!(
                "<tr><td class=\"qname\">{}</td><td>{}</td><td>{}</td></tr>",
                escape(&v_str(f, "name")),
                or_emdash(&escape(&reads)),
                or_emdash(&escape(&writes))
            )
        })
        .collect();
    format!(
        "<h2>File I/O <span class=\"count\">{total}</span></h2>\n<p class=\"muted\">Data files the repository reads and writes, from READ/WRITE and the OPEN mode declared for each.</p>\n{pager_html}\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>File</th><th>Read by</th><th>Written by</th></tr></thead>\n<tbody>{rows}</tbody></table></div>"
    )
}

/// Mirrors `_external_calls`.
fn external_calls_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    let calls_all: Vec<&Value> = cb.arch.get("cobol_external_calls").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    if calls_all.is_empty() {
        return arch_missing("external calls");
    }
    let q = q1(query, "q", "").to_lowercase();
    let calls: Vec<&Value> = calls_all.into_iter().filter(|c| q.is_empty() || v_str(c, "name").to_lowercase().contains(&q)).collect();
    let total = calls.len();
    let (shown, pager_html) = page_slice(&calls, query, PER_PAGE);
    let rows: String = shown
        .iter()
        .map(|c| {
            let called_by = join_str_list(c.get("called_by"), ", ");
            format!(
                "<tr><td class=\"qname\">{}</td><td>{}</td></tr>",
                escape(&v_str(c, "name")),
                or_emdash(&escape(&called_by))
            )
        })
        .collect();
    format!(
        "<h2>External calls <span class=\"count\">{total}</span></h2>\n<p class=\"muted\">Programs CALLed that this repository does not contain. These are the edges out of the bundle,\nand the reason a call graph stops where it does.</p>\n{pager_html}\n<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Program</th><th>Called by</th></tr></thead>\n<tbody>{rows}</tbody></table></div>"
    )
}

// ---------------------------------------------------------------------------
// Knowledge / review (py: lines 1579-1837)
// ---------------------------------------------------------------------------

/// Mirrors `_REVIEW_SORTS` keys + labels (the sort key lambdas themselves are
/// `review_sort_cmp` below, one comparator per entry, ported exactly).
const REVIEW_SORTS: &[(&str, &str)] = &[
    ("confidence", "confidence, highest first"),
    ("confidence_asc", "confidence, lowest first"),
    ("anchor", "anchored first"),
    ("tier", "tier, lowest first"),
    ("title", "title"),
];

/// Mirrors `_claim_kind`: a grouping label for a claim -- the LLM claim
/// kind, else the static rule.
fn claim_kind(item: &Value) -> String {
    let code = item.get("code").cloned().unwrap_or(Value::Object(Map::new()));
    let ck = code.get("claim_kind").and_then(Value::as_str).unwrap_or("");
    if !ck.is_empty() {
        return ck.to_string();
    }
    let d = code.get("derivation").and_then(Value::as_str).unwrap_or("");
    if !d.is_empty() {
        return d.to_string();
    }
    "unclassified".to_string()
}

/// Mirrors `_claim_module`.
fn claim_module(item: &Value) -> String {
    let code = item.get("code").cloned().unwrap_or(Value::Object(Map::new()));
    let qualified = code.get("qualified_name").and_then(Value::as_str).unwrap_or("").to_string();
    if qualified.contains('.') {
        // `qualified.rsplit(".", 1)[0]`: everything before the last dot.
        return qualified.rsplitn(2, '.').last().unwrap_or("").to_string();
    }
    let file = code.get("file").and_then(Value::as_str).unwrap_or("");
    if file.is_empty() { "unknown".to_string() } else { file.to_string() }
}

/// Mirrors `_anchor`.
fn anchor(item: &Value) -> String {
    let code = item.get("code").cloned().unwrap_or(Value::Object(Map::new()));
    let a = code.get("anchor_status").and_then(Value::as_str).unwrap_or("");
    if a.is_empty() { "unknown".to_string() } else { a.to_string() }
}

/// Mirrors the five `_REVIEW_SORTS` sort-key lambdas, reproduced exactly as
/// comparators rather than approximated.
fn review_sort_cmp(sort_by: &str, a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let conf = |v: &Value| v.get("confidence").and_then(Value::as_f64).unwrap_or(0.0);
    let title = |v: &Value| v.get("title").and_then(Value::as_str).unwrap_or("").to_string();
    match sort_by {
        // key = (-confidence, title)
        "confidence" => (-conf(a))
            .partial_cmp(&(-conf(b)))
            .unwrap_or(Ordering::Equal)
            .then_with(|| title(a).cmp(&title(b))),
        // key = (confidence, title)
        "confidence_asc" => conf(a)
            .partial_cmp(&conf(b))
            .unwrap_or(Ordering::Equal)
            .then_with(|| title(a).cmp(&title(b))),
        // key = (anchor(i) != "exact", -confidence)
        "anchor" => {
            let ka = anchor(a) != "exact";
            let kb = anchor(b) != "exact";
            ka.cmp(&kb).then_with(|| (-conf(a)).partial_cmp(&(-conf(b))).unwrap_or(Ordering::Equal))
        }
        // key = (knowledge_tier, title)
        "tier" => {
            let ta = a.get("knowledge_tier").and_then(Value::as_i64).unwrap_or(0);
            let tb = b.get("knowledge_tier").and_then(Value::as_i64).unwrap_or(0);
            ta.cmp(&tb).then_with(|| title(a).cmp(&title(b)))
        }
        // "title": key = title
        _ => title(a).cmp(&title(b)),
    }
}

/// Mirrors `_claim_pool`: the claims one filter names. Every filter but
/// `retired` reads the active pool, because a superseded or retired claim
/// describes an earlier version of the code and there is nothing useful to
/// decide about it. `retired` is the way back to those, so that no claim the
/// Overview counts is unreachable.
fn claim_pool<'a>(items: &'a [Value], flt: &str) -> Vec<&'a Value> {
    if flt == "retired" {
        return items
            .iter()
            .filter(|i| {
                let s = i.get("lifecycle_status").and_then(Value::as_str).unwrap_or("active");
                let s = if s.is_empty() { "active" } else { s };
                s != "active"
            })
            .collect();
    }
    let active = active_claims(items);
    match flt {
        "review_required" => active
            .into_iter()
            .filter(|i| truthy(i.get("review_required")) || i.get("knowledge_tier").and_then(Value::as_i64) == Some(4))
            .collect(),
        "proposed" => active
            .into_iter()
            .filter(|i| i.get("review_status").and_then(Value::as_str) == Some("proposed"))
            .collect(),
        "anchored" => active
            .into_iter()
            .filter(|i| anchor(i) == "exact" && i.get("review_status").and_then(Value::as_str) == Some("proposed"))
            .collect(),
        "unanchored" => active.into_iter().filter(|i| anchor(i) == "llm_claimed").collect(),
        _ => active,
    }
}

/// Mirrors `_knowledge`: claims and their review, as one page.
fn knowledge_view(cb: &CodeBundle, query: &Query, _bundle_dir: &Path) -> String {
    let sel_query = q1(query, "id", "").to_string();
    let mut flt = q1(query, "filter", "review_required").to_string();
    if !CLAIM_FILTERS.contains(&flt.as_str()) {
        flt = "review_required".to_string();
    }
    let group_by = q1(query, "group", "none").to_string();
    let mut sort_by = q1(query, "sort", "confidence").to_string();
    if !REVIEW_SORTS.iter().any(|(s, _)| *s == sort_by) {
        sort_by = "confidence".to_string();
    }
    let tier = q1(query, "tier", "").to_string();
    let q = q1(query, "q", "").to_lowercase();

    let counts: HashMap<&str, usize> =
        CLAIM_FILTERS.iter().map(|name| (*name, claim_pool(&cb.items, name).len())).collect();

    let mut pool = claim_pool(&cb.items, &flt);
    if !tier.is_empty() {
        pool.retain(|i| i.get("knowledge_tier").map(value_display).unwrap_or_else(|| "None".to_string()) == tier);
    }
    if !q.is_empty() {
        pool.retain(|i| v_str(i, "claim").to_lowercase().contains(&q) || v_str(i, "title").to_lowercase().contains(&q));
    }
    pool.sort_by(|a, b| review_sort_cmp(&sort_by, a, b));

    let keep = format!(
        "&group={}&sort={}&tier={}&q={}",
        quote(&group_by),
        quote(&sort_by),
        quote(&tier),
        quote(&q)
    );
    let filter_links: String = CLAIM_FILTERS
        .iter()
        .map(|name| {
            let active = if *name == flt { "active" } else { "" };
            format!(
                "<a class=\"{active}\" href=\"/knowledge?filter={name}{keep}\">{} <span class=\"rail-count\">{}</span></a>",
                name.replace('_', " "),
                counts.get(name).copied().unwrap_or(0)
            )
        })
        .collect();
    let sort_opts: String = REVIEW_SORTS
        .iter()
        .map(|(s, label)| {
            let selected = if *s == sort_by { " selected" } else { "" };
            format!("<option value=\"{s}\"{selected}>{}</option>", escape(label))
        })
        .collect();
    let group_opts: String = [("none", "no grouping"), ("module", "group by module"), ("kind", "group by claim kind")]
        .iter()
        .map(|(g, label)| {
            let selected = if *g == group_by { " selected" } else { "" };
            format!("<option value=\"{g}\"{selected}>{}</option>", escape(label))
        })
        .collect();
    let tier3_sel = if tier == "3" { " selected" } else { "" };
    let tier4_sel = if tier == "4" { " selected" } else { "" };
    let controls = format!(
        "<form class=\"searchbar\" method=\"get\">\n<input type=\"hidden\" name=\"filter\" value=\"{}\">\n<input type=\"text\" name=\"q\" value=\"{}\" placeholder=\"search claim text\">\n<select name=\"tier\"><option value=\"\">any tier</option>\n<option value=\"3\"{tier3_sel}>Tier 3</option>\n<option value=\"4\"{tier4_sel}>Tier 4 (review)</option></select>\n<select name=\"sort\">{sort_opts}</select><select name=\"group\">{group_opts}</select>\n<button>Apply</button></form>",
        escape(&flt),
        escape(&q)
    );

    let sel = if sel_query.is_empty() {
        pool.first().map(|i| v_str(i, "id")).unwrap_or_default()
    } else {
        sel_query
    };
    let listing = claim_list(cb, &pool, &sel, &flt, &keep, &group_by, query);
    let detail = claim_detail(cb, &sel, &flt);
    format!(
        "<h2>Knowledge claims <span class=\"count\">{}</span></h2>\n<nav class=\"segmented filters\">{filter_links}</nav>\n{controls}\n<p class=\"muted\">Tier 3 = behaviour/doc claims. Tier 4 = architecture/workflow/safety, flagged for review.\nDecisions persist to <span class=\"qname\">.codekb/code_reviews.json</span> as a non-destructive overlay; approval is\nnever inferred. Superseded and retired claims are kept out of the other filters, because they describe an\nearlier version of the code; the <a href=\"/knowledge?filter=retired\">retired</a> filter lists them.</p>\n<div class=\"review-layout\">{listing}{detail}</div>{REVIEW_BATCH_SCRIPT}",
        pool.len()
    )
}

/// Mirrors `_claim_list`: one list that both selects and navigates. The
/// checkbox selects for a batch decision; the title opens the claim.
fn claim_list(
    _cb: &CodeBundle,
    pool: &[&Value],
    sel: &str,
    flt: &str,
    keep: &str,
    group_by: &str,
    query: &Query,
) -> String {
    if pool.is_empty() {
        return "<div class=\"panel\"><p class=\"muted\">Nothing in this queue.</p></div>".to_string();
    }
    let (shown, pager_html) = page_slice(pool, query, PER_PAGE);
    let key_fn: Option<fn(&Value) -> String> = match group_by {
        "module" => Some(claim_module),
        "kind" => Some(claim_kind),
        _ => None,
    };
    // Python's `dict.setdefault` preserves first-seen-group order; a plain
    // `HashMap` would not, so this walks `shown` building an ordered
    // `Vec<(group, items)>` instead.
    let mut groups: Vec<(String, Vec<&Value>)> = Vec::new();
    for item in shown.iter().copied() {
        let g = key_fn.map(|f| f(item)).unwrap_or_default();
        if let Some(entry) = groups.iter_mut().find(|(k, _)| k == &g) {
            entry.1.push(item);
        } else {
            groups.push((g, vec![item]));
        }
    }
    let row = |item: &Value, group: &str| -> String {
        let iid = v_str(item, "id");
        let status = v_str(item, "review_status");
        let tier = item.get("knowledge_tier").map(value_display).unwrap_or_default();
        let mut marks = format!("<span class=\"tag\">T{tier}</span>");
        if anchor(item) == "llm_claimed" {
            marks.push_str(" <span class=\"tag tier4\">unanchored</span>");
        }
        let active = if iid == sel { " active" } else { "" };
        let confidence = item.get("confidence").map(value_display).unwrap_or_default();
        format!(
            "<div class=\"claim-row{active}\"><input type=\"checkbox\" name=\"item_ids\" value=\"{}\" data-group=\"{}\" aria-label=\"select claim\"><a href=\"/knowledge?filter={}{keep}&id={}\">{}</a><span class=\"claim-meta\">{marks} <span class=\"{}\">{}</span> \u{b7} conf {confidence}</span></div>",
            escape(&iid),
            escape(group),
            escape(flt),
            quote(&iid),
            escape(&v_str(item, "title")),
            escape(&status),
            escape(&status)
        )
    };
    let blocks: String = groups
        .iter()
        .map(|(group, entries)| {
            let header = if !group.is_empty() {
                format!(
                    "<p class=\"claim-group\"><label class=\"choice\"><input type=\"checkbox\" data-select-all=\"{}\"><strong>{}</strong> <span class=\"count\">{}</span></label></p>",
                    escape(group),
                    escape(group),
                    entries.len()
                )
            } else {
                String::new()
            };
            let rows: String = entries.iter().map(|item| row(item, group)).collect();
            format!("{header}{rows}")
        })
        .collect();
    format!(
        "<form id=\"review-batch\" method=\"post\" action=\"/review\">\n<input type=\"hidden\" name=\"filter\" value=\"{}\">\n<p><label class=\"choice\"><input type=\"checkbox\" data-select-all=\"\"><strong>Select all shown</strong></label></p>\n<div class=\"rail claim-rail\">{blocks}</div>\n{pager_html}\n<div id=\"batch-bar\" class=\"batch-bar hidden\">\n  <p><strong><span id=\"review-batch-count\">0</span> selected</strong> &mdash; one decision, applied to every one.</p>\n  <label>Reviewer<input name=\"reviewer\" value=\"local:user\"></label>\n  <label>Rationale<input name=\"rationale\" placeholder=\"why these are approved / deferred / rejected\" required></label>\n  <div class=\"toolbar\">\n    <button data-batch name=\"action\" value=\"approve\">Approve selected</button>\n    <button data-batch class=\"secondary\" name=\"action\" value=\"defer\">Defer</button>\n    <button data-batch class=\"secondary\" name=\"action\" value=\"reject\">Reject</button>\n  </div>\n  <p class=\"muted\">One event is recorded per claim, marked as part of a batch, so the history stays per-claim.</p>\n</div></form>",
        escape(flt)
    )
}

/// Mirrors `_claim_detail`.
fn claim_detail(cb: &CodeBundle, sel: &str, flt: &str) -> String {
    let Some(&idx) = cb.item_by_id.get(sel) else {
        return "<div class=\"panel\">No claim selected.</div>".to_string();
    };
    let it = &cb.items[idx];
    let ev_ids: Vec<String> = it
        .get("evidence")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let excerpt = ev_ids
        .first()
        .and_then(|eid| cb.evidence_by_id.get(eid))
        .map(|&i| {
            let truncated: String = v_str(&cb.evidence[i], "excerpt").chars().take(1600).collect();
            escape(&truncated)
        })
        .unwrap_or_else(|| "(no evidence)".to_string());
    let events: Vec<&Value> = cb.review_events.iter().filter(|e| v_str(e, "item_id") == sel).collect();
    let ev_rows: String = events
        .iter()
        .map(|e| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(&v_str(e, "action")),
                escape(&v_str(e, "reviewer")),
                escape(&v_str(e, "rationale"))
            )
        })
        .collect();
    let sym = it
        .get("symbols")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let history = if !events.is_empty() {
        format!(
            "<h3 style=\"margin-top:12px\">Review history</h3><div class=\"table-wrap\"><table class=\"code-table\"><thead><tr><th>Action</th><th>Reviewer</th><th>Rationale</th></tr></thead><tbody>{ev_rows}</tbody></table></div>"
        )
    } else {
        String::new()
    };
    let tier = it.get("knowledge_tier").and_then(Value::as_i64).unwrap_or(0);
    let tier4 = if tier == 4 { " tier4" } else { "" };
    let confidence = it.get("confidence").map(value_display).unwrap_or_default();
    format!(
        "<div class=\"panel\"><div class=\"detail-header\"><h3>{}</h3>\n<span class=\"tag{tier4}\">Tier {tier}</span></div>\n<dl class=\"kv\"><dt>Status</dt><dd>{}</dd>\n<dt>Symbol</dt><dd>{}</dd><dt>Confidence</dt><dd>{confidence}</dd>\n<dt>Anchor</dt><dd>{}</dd></dl>\n<h3 style=\"margin-top:12px\">Claim</h3><pre>{}</pre>\n<h3>Evidence</h3><pre>{excerpt}</pre>\n<form class=\"inline-form\" method=\"post\" action=\"/review\">\n<input type=\"hidden\" name=\"item_id\" value=\"{}\"><input type=\"hidden\" name=\"filter\" value=\"{}\">\n<label>Reviewer<input name=\"reviewer\" value=\"local:user\"></label>\n<label>Rationale<input name=\"rationale\" placeholder=\"why approve / defer / reject\" required></label>\n<div class=\"row-actions\">\n<button name=\"action\" value=\"approve\">Approve</button>\n<button class=\"secondary\" name=\"action\" value=\"defer\">Defer</button>\n<button class=\"secondary\" name=\"action\" value=\"reject\">Reject</button>\n<button class=\"secondary\" name=\"action\" value=\"comment\">Comment</button></div></form>\n{history}</div>",
        escape(&v_str(it, "title")),
        rstatus(&v_str(it, "review_status")),
        link_symbol(cb, &sym, ""),
        escape(&anchor(it)),
        escape(&v_str(it, "claim")),
        escape(sel),
        escape(flt)
    )
}

// ---------------------------------------------------------------------------
// Search (py: lines 1840-1972)
// ---------------------------------------------------------------------------

/// Mirrors `_search`: one box over everything the bundle holds.
fn search_view(cb: &CodeBundle, query: &Query, bundle_dir: &Path) -> String {
    let term = q1(query, "q", "").trim().to_string();
    if term.is_empty() {
        return "<h2>Search</h2><div class=\"panel\"><p class=\"muted\">Search symbols, claims, endpoints, files and documents in this bundle at once.</p></div>".to_string();
    }
    let groups = search_all(cb, bundle_dir, &term);
    let total: usize = groups.iter().map(|(_, _, hits)| hits.len()).sum();
    if total == 0 {
        return format!(
            "<h2>Search <span class=\"count\">0</span></h2><div class=\"panel\"><p>Nothing matched <strong>{}</strong>.</p><p class=\"muted\">Symbols match on qualified name, claims on title and text, endpoints on path and handler, files on path, and documents on their filename.</p></div>",
            escape(&term)
        );
    }
    let mut blocks = String::new();
    for (label, section_href, hits) in &groups {
        if hits.is_empty() {
            continue;
        }
        let rows: String = hits
            .iter()
            .take(10)
            .map(|(title, href, meta)| {
                format!("<div class=\"hit\"><a href=\"{href}\">{}</a><span class=\"hit-meta\">{meta}</span></div>", escape(title))
            })
            .collect();
        let more = if hits.len() > 10 {
            format!(
                "<p class=\"muted\"><a href=\"{section_href}&q={}\">See all {} in {}</a></p>",
                quote(&term),
                hits.len(),
                escape(&label.to_lowercase())
            )
        } else {
            String::new()
        };
        blocks.push_str(&format!(
            "<div class=\"panel\"><h3>{} <span class=\"count\">{}</span></h3>{rows}{more}</div>",
            escape(label),
            hits.len()
        ));
    }
    format!(
        "<h2>Search <span class=\"count\">{total}</span></h2><p class=\"muted\">Results for <strong>{}</strong>, grouped by what they are.</p>{blocks}",
        escape(&term)
    )
}

/// Mirrors `_search_all`: `(group label, section link, [(title, href,
/// meta)])` for one term.
fn search_all(cb: &CodeBundle, bundle_dir: &Path, term: &str) -> Vec<(String, String, Vec<(String, String, String)>)> {
    let needle = term.to_lowercase();

    let symbols: Vec<(String, String, String)> = cb
        .symbols
        .iter()
        .filter(|s| v_str(s, "qualified_name").to_lowercase().contains(&needle))
        .map(|s| {
            (
                v_str(s, "qualified_name"),
                format!("/code?mode=symbol&id={}", quote(&v_str(s, "id"))),
                format!(
                    "<span class=\"tag\">{}</span> <span class=\"muted\">{}:{}</span>",
                    escape(&v_str(s, "kind")),
                    escape(&v_str(s, "file")),
                    v_i64(s, "line_start")
                ),
            )
        })
        .collect();

    let claims: Vec<(String, String, String)> = cb
        .items
        .iter()
        .filter(|i| v_str(i, "title").to_lowercase().contains(&needle) || v_str(i, "claim").to_lowercase().contains(&needle))
        .map(|i| {
            let title = v_str(i, "title");
            let title = if title.is_empty() { v_str(i, "claim").chars().take(80).collect() } else { title };
            let status = v_str(i, "review_status");
            (
                title,
                format!("/knowledge?filter=all&id={}", quote(&v_str(i, "id"))),
                format!(
                    "<span class=\"tag\">T{}</span> <span class=\"status {}\">{}</span>",
                    i.get("knowledge_tier").map(value_display).unwrap_or_default(),
                    escape(&status),
                    escape(&status)
                ),
            )
        })
        .collect();

    let endpoints: Vec<(String, String, String)> = cb
        .endpoints
        .iter()
        .filter(|e| v_str(e, "path").to_lowercase().contains(&needle) || v_str(e, "handler_qname").to_lowercase().contains(&needle))
        .map(|e| {
            (
                format!("{} {}", v_str(e, "method"), v_str(e, "path")),
                format!("/architecture?mode=endpoints&q={}", quote(&v_str(e, "path"))),
                format!("<span class=\"muted\">{}</span>", escape(&v_str(e, "handler"))),
            )
        })
        .collect();

    let files: Vec<(String, String, String)> = cb
        .sources
        .iter()
        .filter(|s| v_str(s, "path").to_lowercase().contains(&needle))
        .map(|s| {
            (
                v_str(s, "path"),
                format!("/code?mode=files&id={}", quote(&v_str(s, "id"))),
                format!("<span class=\"tag\">{}</span>", escape(&v_str(s, "language"))),
            )
        })
        .collect();

    let docs: Vec<(String, String, String)> = bundle_docs(bundle_dir)
        .into_iter()
        .filter(|rel| rel.to_lowercase().contains(&needle))
        .map(|rel| {
            let href = format!("/docs?path={}", quote(&rel));
            (rel, href, "<span class=\"muted\">document</span>".to_string())
        })
        .collect();

    vec![
        ("Symbols".to_string(), "/code?mode=symbols".to_string(), symbols),
        ("Claims".to_string(), "/knowledge?filter=all".to_string(), claims),
        ("Endpoints".to_string(), "/architecture?mode=endpoints".to_string(), endpoints),
        ("Files".to_string(), "/code?mode=files".to_string(), files),
        ("Documents".to_string(), "/docs?path=index.md".to_string(), docs),
    ]
}

/// Mirrors `_bundle_docs`: Markdown in the bundle, as bundle-relative posix
/// paths.
fn bundle_docs(bundle_dir: &Path) -> Vec<String> {
    let mut found: Vec<std::path::PathBuf> = walkdir::WalkDir::new(bundle_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().map(|x| x == "md").unwrap_or(false))
        .map(|e| e.path().to_path_buf())
        .collect();
    found.sort();
    let mut out = Vec::new();
    for path in found {
        if let Ok(rel) = path.strip_prefix(bundle_dir) {
            let rel_posix = rel.to_string_lossy().replace('\\', "/");
            if !rel_posix.starts_with(".codekb/") {
                out.push(rel_posix);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Agent (py: lines 1975-2103)
// ---------------------------------------------------------------------------

/// Mirrors `_agent`. WIRING: `code_context` (from `.context`, not yet
/// ported) is called inside a try/except in Python that this mirrors with a
/// `match` -- any error renders inline, it never propagates.
fn agent_view(cb: &CodeBundle, query: &Query, bundle_dir: &Path) -> String {
    let task = q1(query, "task", "explain").to_string();
    let q = q1(query, "query", "").to_string();
    let mut result_html = String::new();
    if !q.is_empty() {
        match code_context(bundle_dir, &task, Some(&q), None) {
            Ok(ctx) => result_html = agent_result(cb, &ctx),
            Err(exc) => {
                result_html = format!("<div class=\"panel\"><h3>Error</h3><pre>{}</pre></div>", escape(&format!("{exc:?}")));
            }
        }
    }
    let form = format!(
        "<div class=\"panel\"><h3>Ask the code KB</h3>\n<form class=\"inline-form\" method=\"get\" action=\"/agent\">\n<label>Task id<input name=\"task\" value=\"{}\" placeholder=\"explain / plan-change / find\"></label>\n<label>Query<input name=\"query\" value=\"{}\" placeholder=\"e.g. transcript batch processing\" required></label>\n<button>Retrieve context</button></form>\n<p class=\"muted\">Runs the real <span class=\"qname\">code_context</span> retrieval: matched symbols, evidence, relations, and usable knowledge \u{2014} with warnings for unresolved/review-required items.</p></div>",
        escape(&task),
        escape(&q)
    );
    let body = if result_html.is_empty() {
        "<div class=\"panel muted\">Enter a query to retrieve grounded context.</div>".to_string()
    } else {
        result_html
    };
    format!("<h2>Agent context</h2><div class=\"agent-layout\">{form}<div>{body}</div></div>")
}

/// Mirrors `_agent_result`.
fn agent_result(cb: &CodeBundle, ctx: &Value) -> String {
    let sect = |title: &str, rows: &[String], cols: &[&str]| -> String {
        let head: String = cols.iter().map(|c| format!("<th>{}</th>", escape(c))).collect();
        format!(
            "<div class=\"panel\"><h3>{} <span class=\"count\">{}</span></h3><div class=\"table-wrap\"><table class=\"code-table\"><thead><tr>{head}</tr></thead><tbody>{}</tbody></table></div></div>",
            escape(title),
            rows.len(),
            rows.concat()
        )
    };
    let task = ctx.get("task").cloned().unwrap_or(Value::Null);
    let (task_id, query_str) = if task.is_object() {
        (v_str(&task, "id"), v_str(&task, "query"))
    } else {
        (value_display(&task), String::new())
    };
    let sym_ref = |value: &str| -> String {
        if cb.symbol_by_id.contains_key(value) {
            link_symbol(cb, value, "")
        } else {
            format!("<span class=\"qname muted\">{}</span>", escape(value))
        }
    };

    let syms: Vec<&Value> = ctx
        .get("symbols")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter(|v| v.is_object()).collect())
        .unwrap_or_default();
    let sym_rows: Vec<String> = syms
        .iter()
        .map(|s| {
            format!(
                "<tr><td>{}</td><td><span class=\"tag\">{}</span></td><td class=\"qname muted\">{}:{}</td></tr>",
                link_symbol(cb, &v_str(s, "id"), ""),
                escape(&v_str(s, "kind")),
                escape(&v_str(s, "file")),
                v_i64(s, "line_start")
            )
        })
        .collect();

    let rels: Vec<&Value> = ctx
        .get("relations")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter(|v| v.is_object()).collect())
        .unwrap_or_default();
    let rel_rows: Vec<String> = rels
        .iter()
        .take(40)
        .map(|r| {
            let relation = r.get("relation").cloned();
            let status_html = match &relation {
                Some(rel) if rel.is_object() => rstatus(&v_str(rel, "resolution_status")),
                _ => String::new(),
            };
            format!(
                "<tr><td>{}</td><td><span class=\"tag\">{}</span></td><td>{}</td><td>{status_html}</td></tr>",
                sym_ref(&v_str(r, "subject")),
                escape(&v_str(r, "predicate")),
                sym_ref(&v_str(r, "object"))
            )
        })
        .collect();
    let rel_note = if rels.len() > 40 { format!(" (first 40 of {})", rels.len()) } else { String::new() };

    let know: Vec<&Value> = ctx
        .get("code_knowledge")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter(|v| v.is_object()).collect())
        .unwrap_or_default();
    let know_rows: Vec<String> = know
        .iter()
        .map(|k| {
            let claim: String = v_str(k, "claim").chars().take(320).collect();
            let tier = k.get("knowledge_tier").and_then(Value::as_i64).unwrap_or(0);
            let tier4 = if tier == 4 { " tier4" } else { "" };
            format!(
                "<tr><td>{}</td><td><span class=\"tag{tier4}\">T{tier}</span></td><td>{}</td></tr>",
                escape(&claim),
                rstatus(&v_str(k, "review_status"))
            )
        })
        .collect();

    let tests: Vec<&Value> = ctx
        .get("tests")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter(|v| v.is_object()).collect())
        .unwrap_or_default();
    let test_rows: Vec<String> = tests
        .iter()
        .map(|t| {
            format!(
                "<tr><td>{}</td><td class=\"qname muted\">{}:{}</td></tr>",
                link_symbol(cb, &v_str(t, "id"), ""),
                escape(&v_str(t, "file")),
                v_i64(t, "line_start")
            )
        })
        .collect();

    let warns: Vec<Value> = ctx.get("warnings").and_then(Value::as_array).cloned().unwrap_or_default();
    let rules: Vec<Value> = ctx.get("context_rules").and_then(Value::as_array).cloned().unwrap_or_default();

    let mut blocks = vec![format!(
        "<div class=\"panel\"><h3>Retrieval</h3><dl class=\"kv\"><dt>Task</dt><dd>{}</dd><dt>Query</dt><dd>{}</dd><dt>Found</dt><dd>{} symbol(s), {} relation(s), {} claim(s), {} test(s)</dd></dl></div>",
        escape(&task_id),
        escape(&query_str),
        syms.len(),
        rels.len(),
        know.len(),
        tests.len()
    )];
    if syms.is_empty() {
        blocks.push("<div class=\"panel muted\">No symbols matched this query. Try a term that appears in a function/class/module name (e.g. \"batch\", \"transcript\", \"snowflake\").</div>".to_string());
    }
    if !sym_rows.is_empty() {
        blocks.push(sect("Matched symbols", &sym_rows, &["Symbol", "Kind", "Location"]));
    }
    if !know_rows.is_empty() {
        blocks.push(sect("Knowledge claims", &know_rows, &["Claim", "Tier", "Review"]));
    }
    if !rel_rows.is_empty() {
        blocks.push(sect(&format!("Relations{rel_note}"), &rel_rows, &["Subject", "Predicate", "Object", "Status"]));
    }
    if !test_rows.is_empty() {
        blocks.push(sect("Tests", &test_rows, &["Test", "Location"]));
    }
    if !warns.is_empty() {
        let items: String = warns.iter().take(20).map(|w| format!("<li class=\"muted\">{}</li>", escape(&value_display(w)))).collect();
        blocks.push(format!("<div class=\"panel\"><h3>Warnings</h3><ul>{items}</ul></div>"));
    }
    if !rules.is_empty() {
        let items: String = rules.iter().map(|r| format!("<li class=\"muted\">{}</li>", escape(&value_display(r)))).collect();
        blocks.push(format!("<div class=\"panel\"><h3>Context rules</h3><ul>{items}</ul></div>"));
    }
    let json_str = serde_json::to_string_pretty(ctx).unwrap_or_default();
    let truncated: String = json_str.chars().take(14000).collect();
    let raw = format!("<details class=\"panel\"><summary>Raw context JSON</summary><pre>{}</pre></details>", escape(&truncated));
    format!("{}{raw}", blocks.concat())
}

// ---------------------------------------------------------------------------
// Docs / Markdown rendering (py: lines 2109-2368)
// ---------------------------------------------------------------------------

/// Python `posixpath.normpath` equivalent: purely lexical, no filesystem
/// access.
fn posix_normpath(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let leading_slashes = if path.starts_with("//") && !path.starts_with("///") {
        2
    } else if path.starts_with('/') {
        1
    } else {
        0
    };
    let mut comps: Vec<&str> = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part != ".." {
            comps.push(part);
        } else if leading_slashes == 0 && (comps.is_empty() || *comps.last().unwrap() == "..") {
            comps.push("..");
        } else if !comps.is_empty() {
            comps.pop();
        }
    }
    let mut result = "/".repeat(leading_slashes) + &comps.join("/");
    if result.is_empty() {
        result = ".".to_string();
    }
    result
}

/// Python `posixpath.dirname`.
fn posix_dirname(path: &str) -> String {
    match path.rfind('/') {
        Some(idx) => {
            let head = &path[..idx];
            let trimmed = head.trim_end_matches('/');
            if trimmed.is_empty() { "/".to_string() } else { trimmed.to_string() }
        }
        None => String::new(),
    }
}

/// Python `posixpath.join(a, b)` for the two-argument case used here.
fn posix_join(a: &str, b: &str) -> String {
    if b.starts_with('/') {
        return b.to_string();
    }
    if a.is_empty() || a.ends_with('/') { format!("{a}{b}") } else { format!("{a}/{b}") }
}

/// Mirrors `_DOCS_RAIL`'s companion function `_docs_rail`.
///
/// CONFIRMED against the real Python source (re-queried via tools-code MCP
/// `code_context` with a line-range-specific task after an earlier batch hit
/// a genuine id collision between `_DOCS_RAIL`, the constant at lines
/// 2109-2119, and `_docs_rail`, the function at lines 2138-2171, both
/// sharing one `symbol_id`/`evidence_id` in the MCP index):
///
/// ```python
/// def _docs_rail(bundle_dir: Path, current: str) -> str:
///     def link(entry: str, label: str) -> str:
///         css = "active" if entry == current else ""
///         return f'<a class="{css}" href="/docs?path={quote(entry)}">{escape(label)}</a>'
///
///     groups = [("Guides", [(e, l) for e, l in _DOCS_RAIL if (bundle_dir / e).exists()])]
///     reports_dir = bundle_dir / "reports"
///     if reports_dir.is_dir():
///         found = sorted(reports_dir.glob("*.md"))
///         groups.append(
///             (
///                 "Reports",
///                 [
///                     (f"reports/{p.name}", p.stem.replace("_", " ").capitalize())
///                     for p in found
///                 ],
///             )
///         )
///
///     out = []
///     for title, entries in groups:
///         if entries:
///             out.append(
///                 f'<p class="rail-group">{escape(title)}</p>'
///                 + "".join(link(e, l) for e, l in entries)
///             )
///     return "".join(out)
/// ```
///
/// Two things the earlier reconstruction got wrong, now fixed: the group
/// heading tag is `<p class="rail-group">`, not `<div ...>`, and the report
/// stem is de-slugged with `.replace("_", " ")` (underscore), not `-`
/// (hyphen).
fn docs_rail(bundle_dir: &Path, current: &str) -> String {
    let link = |entry: &str, label: &str| -> String {
        let active = if entry == current { "active" } else { "" };
        format!("<a class=\"{active}\" href=\"/docs?path={}\">{}</a>", quote(entry), escape(label))
    };
    let mut groups: Vec<(&str, String)> = Vec::new();
    let guides: String = DOCS_RAIL
        .iter()
        .filter(|(entry, _)| bundle_dir.join(entry).exists())
        .map(|(entry, label)| link(entry, label))
        .collect();
    groups.push(("Guides", guides));
    let reports_dir = bundle_dir.join("reports");
    if reports_dir.is_dir() {
        let mut report_paths: Vec<std::path::PathBuf> = std::fs::read_dir(&reports_dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().map(|x| x == "md").unwrap_or(false))
                    .collect()
            })
            .unwrap_or_default();
        report_paths.sort();
        let reports: String = report_paths
            .iter()
            .map(|p| {
                let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let label = python_capitalize(&stem.replace('_', " "));
                let file_name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let entry = format!("reports/{file_name}");
                link(&entry, &label)
            })
            .collect();
        groups.push(("Reports", reports));
    }
    groups
        .into_iter()
        .filter(|(_, links)| !links.is_empty())
        .map(|(label, links)| format!("<p class=\"rail-group\">{}</p>{links}", escape(label)))
        .collect()
}

/// Mirrors `_docs`.
fn docs_view(cb: &CodeBundle, query: &Query, bundle_dir: &Path) -> String {
    let _ = cb;
    let default = if bundle_dir.join("overview.md").exists() { "overview.md" } else { "index.md" };
    let requested = q1(query, "path", default).trim().replace('\\', "/");
    let norm = posix_normpath(&requested);
    // Path-traversal / type guard: must stay inside the bundle and be an
    // existing .md file. `Path::join` with an absolute second operand
    // replaces the whole path in Rust exactly like Python's `pathlib`
    // `/` operator does, so an absolute `norm` is caught the same way an
    // escaping `..` prefix is: both fail this purely-lexical inside check
    // without ever touching the filesystem.
    let inside = !norm.starts_with('/') && norm != ".." && !norm.starts_with("../");
    let target = bundle_dir.join(&norm);
    let rail = docs_rail(bundle_dir, &norm);
    if !inside || !norm.ends_with(".md") || !target.is_file() {
        let detail = format!("<div class=\"panel\">Document not found: <code>{}</code></div>", escape(&norm));
        return format!("<h2>Docs</h2><div class=\"split\"><div class=\"rail\">{rail}</div>{detail}</div>");
    }
    let text = std::fs::read_to_string(&target).unwrap_or_default();
    if !q1(query, "raw", "").is_empty() {
        let truncated: String = text.chars().take(200_000).collect();
        let body = format!("<div class=\"panel\"><pre>{}</pre></div>", escape(&truncated));
        return format!("<h2>Docs</h2><div class=\"split\"><div class=\"rail\">{rail}</div>{body}</div>");
    }
    let (content, has_mermaid) = markdown_to_html(&text, &norm);
    let crumb = docs_breadcrumb(&norm);
    let raw_link = format!("<a class=\"button-link secondary\" href=\"/docs?path={}&raw=1\">raw</a>", quote(&norm));
    let detail = format!(
        "<div class=\"panel\"><div class=\"detail-header\"><div class=\"muted qname\">{crumb}</div>{raw_link}</div><div class=\"docbody\">{content}</div></div>"
    );
    let script = if has_mermaid { MERMAID_SNIPPET } else { "" };
    format!("{DOCS_STYLE}<h2>Docs</h2><div class=\"split\"><div class=\"rail\">{rail}</div>{detail}</div>{script}")
}

/// Mirrors `_docs_breadcrumb`.
fn docs_breadcrumb(norm: &str) -> String {
    let parts: Vec<&str> = norm.split('/').collect();
    let mut crumbs = Vec::new();
    for (idx, part) in parts.iter().enumerate() {
        if idx < parts.len() - 1 {
            let acc = parts[..=idx].join("/");
            let index_path = format!("{acc}/index.md");
            crumbs.push(format!("<a href=\"/docs?path={}\">{}</a>", quote(&index_path), escape(part)));
        } else {
            crumbs.push(escape(part));
        }
    }
    crumbs.join(" / ")
}

/// Mirrors `_markdown_to_html`.
fn markdown_to_html(text: &str, current_path: &str) -> (String, bool) {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut fm_html = String::new();
    let mut start = 0usize;
    if lines.first().map(|l| l.trim() == "---").unwrap_or(false) {
        for i in 1..lines.len() {
            if lines[i].trim() == "---" {
                let fm = lines[1..i].join("\n");
                fm_html = format!("<details class=\"fm\"><summary>frontmatter</summary><pre>{}</pre></details>", escape(&fm));
                start = i + 1;
                break;
            }
        }
    }
    let body: &[&str] = if start <= lines.len() { &lines[start..] } else { &[] };
    let mut parts: Vec<String> = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut has_mermaid = false;

    let heading_re = regex::Regex::new(r"^(#{1,6})\s+(.*)$").unwrap();
    let list_re = regex::Regex::new(r"^[-*]\s+").unwrap();
    let list_indent_re = regex::Regex::new(r"^\s*[-*]\s+").unwrap();

    let mut i = 0usize;
    let n = body.len();
    while i < n {
        let raw = body[i];
        let line = raw.trim();
        if line.starts_with("```") {
            if !para.is_empty() {
                parts.push(format!("<p>{}</p>", md_inline(&para.join(" "), current_path)));
                para.clear();
            }
            let lang = line[3..].trim().to_string();
            let mut code: Vec<&str> = Vec::new();
            i += 1;
            while i < n && !body[i].trim().starts_with("```") {
                code.push(body[i]);
                i += 1;
            }
            i += 1;
            let joined = code.join("\n");
            if lang == "mermaid" {
                has_mermaid = true;
                parts.push(format!("<pre class=\"mermaid\">{}</pre>", escape(&joined)));
            } else {
                parts.push(format!("<pre><code>{}</code></pre>", escape(&joined)));
            }
            continue;
        }
        if line.is_empty() {
            if !para.is_empty() {
                parts.push(format!("<p>{}</p>", md_inline(&para.join(" "), current_path)));
                para.clear();
            }
            i += 1;
            continue;
        }
        if let Some(caps) = heading_re.captures(line) {
            if !para.is_empty() {
                parts.push(format!("<p>{}</p>", md_inline(&para.join(" "), current_path)));
                para.clear();
            }
            let level = caps.get(1).unwrap().as_str().len();
            let rest = caps.get(2).unwrap().as_str();
            parts.push(format!("<h{level}>{}</h{level}>", md_inline(rest, current_path)));
            i += 1;
            continue;
        }
        if line.contains('|') && i + 1 < n && md_is_table_sep(body[i + 1]) {
            if !para.is_empty() {
                parts.push(format!("<p>{}</p>", md_inline(&para.join(" "), current_path)));
                para.clear();
            }
            let header = md_split_row(line);
            i += 2;
            let mut rows: Vec<Vec<String>> = Vec::new();
            while i < n && !body[i].trim().is_empty() && body[i].contains('|') {
                rows.push(md_split_row(body[i]));
                i += 1;
            }
            let thead: String = header.iter().map(|c| format!("<th>{}</th>", md_inline(c, current_path))).collect();
            let tbody: String = rows
                .iter()
                .map(|row| {
                    format!(
                        "<tr>{}</tr>",
                        row.iter().map(|c| format!("<td>{}</td>", md_inline(c, current_path))).collect::<String>()
                    )
                })
                .collect();
            parts.push(format!(
                "<div class=\"table-wrap\"><table class=\"code-table\"><thead><tr>{thead}</tr></thead><tbody>{tbody}</tbody></table></div>"
            ));
            continue;
        }
        if list_re.is_match(line) {
            if !para.is_empty() {
                parts.push(format!("<p>{}</p>", md_inline(&para.join(" "), current_path)));
                para.clear();
            }
            let mut items = String::new();
            while i < n && list_indent_re.is_match(body[i]) {
                let item = list_indent_re.replace(body[i], "");
                items.push_str(&format!("<li>{}</li>", md_inline(&item, current_path)));
                i += 1;
            }
            parts.push(format!("<ul>{items}</ul>"));
            continue;
        }
        para.push(line.to_string());
        i += 1;
    }
    if !para.is_empty() {
        parts.push(format!("<p>{}</p>", md_inline(&para.join(" "), current_path)));
    }
    (format!("{fm_html}{}", parts.concat()), has_mermaid)
}

/// Mirrors `_md_is_table_sep`.
fn md_is_table_sep(line: &str) -> bool {
    let token = line.trim();
    if token.is_empty() {
        return false;
    }
    let allowed: HashSet<char> = "|-: ".chars().collect();
    token.chars().all(|c| allowed.contains(&c)) && token.contains('-')
}

/// Mirrors `_md_split_row`.
fn md_split_row(line: &str) -> Vec<String> {
    line.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect()
}

/// Mirrors `_md_inline`.
fn md_inline(text: &str, current_path: &str) -> String {
    let mut escaped = escape(text);
    let mut stash: Vec<(String, String)> = Vec::new();

    let code_re = regex::Regex::new(r"`([^`]+)`").unwrap();
    escaped = code_re
        .replace_all(&escaped, |caps: &regex::Captures| {
            let token = format!("\u{0}{}\u{0}", stash.len());
            stash.push((token.clone(), format!("<code>{}</code>", &caps[1])));
            token
        })
        .into_owned();

    let link_re = regex::Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
    escaped = link_re
        .replace_all(&escaped, |caps: &regex::Captures| {
            let token = format!("\u{0}{}\u{0}", stash.len());
            stash.push((token.clone(), md_link(&caps[1], &caps[2], current_path)));
            token
        })
        .into_owned();

    let bold_re = regex::Regex::new(r"\*\*([^*]+)\*\*").unwrap();
    escaped = bold_re.replace_all(&escaped, "<strong>$1</strong>").into_owned();

    for (token, fragment) in &stash {
        escaped = escaped.replace(token, fragment);
    }
    escaped
}

/// Mirrors `_md_link`.
fn md_link(label: &str, href: &str, current_path: &str) -> String {
    let label_html = escape(label);
    let href = href.trim();
    if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("mailto:") || href.starts_with('#') {
        return format!("<a href=\"{}\" target=\"_blank\" rel=\"noopener\">{label_html}</a>", escape(href));
    }
    let (mut href, mut anchor_frag) = (href.to_string(), String::new());
    if let Some(idx) = href.find('#') {
        anchor_frag = href[idx + 1..].to_string();
        href.truncate(idx);
    }
    if href.is_empty() {
        return label_html;
    }
    let resolved = posix_normpath(&posix_join(&posix_dirname(current_path), &href));
    if resolved.ends_with(".md") {
        let fragment = if !anchor_frag.is_empty() { format!("#{}", quote(&anchor_frag)) } else { String::new() };
        format!("<a href=\"/docs?path={}{fragment}\">{label_html}</a>", quote(&resolved))
    } else {
        format!("{label_html} <span class=\"muted qname\">({})</span>", escape(&resolved))
    }
}

// ---------------------------------------------------------------------------
// Form handlers (py: lines 2374-2478)
// ---------------------------------------------------------------------------

/// Python `Path(s).expanduser()`: expands a leading `~` against the user's
/// home directory.
fn expand_user(path: &str) -> std::path::PathBuf {
    if path == "~" || path.starts_with("~/") || path.starts_with("~\\") {
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            let rest = path.strip_prefix('~').unwrap_or("").trim_start_matches(['/', '\\']);
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

/// Mirrors `handle_code_pipeline_form`: start a pipeline run for a
/// repository folder path. Returns as soon as the run is queued.
///
/// WIRING: `rebuild_risk`/`start_code_bundle_build` (from `.pipeline`, not
/// yet ported) are called unguarded, matching Python's own unguarded calls.
/// `crate::model::invalidate_code_bundle_cache` does not exist yet -- see
/// the file-level WIRING note; without it, this diverges from Python's
/// `_CACHE.pop(str(bundle_dir), None)`.
pub fn handle_code_pipeline_form(bundle_dir: &Path, form: &HashMap<String, String>) -> Result<Value> {
    let repo_value = form.get("repo_dir").map(|s| s.trim()).unwrap_or("");
    if repo_value.is_empty() {
        bail!("repository folder path is required");
    }
    let repo_dir = expand_user(repo_value);
    if !repo_dir.exists() || !repo_dir.is_dir() {
        bail!("repository directory does not exist: {}", repo_dir.display());
    }
    let repo_dir_str = repo_dir.to_string_lossy().to_string();

    // Only demand confirmation for the case that actually loses something,
    // and name both repositories so the warning is checkable rather than
    // generic.
    let risk = rebuild_risk(bundle_dir, &repo_dir_str)?;
    let confirmed = form.get("confirm_bundle_update").map(|s| s.as_str()) == Some("on");
    if truthy(risk.get("confirm_required")) && !confirmed {
        bail!(
            "this bundle was built from {}, and {} is a different repository. Rebuilding retires all {} existing claim(s). Confirm the change before running the pipeline.",
            v_str_or(&risk, "recorded_repo", ""),
            repo_dir.display(),
            v_i64(&risk, "active_claims")
        );
    }

    let mining_mode = {
        let m = form.get("mining_mode").map(|s| s.as_str()).unwrap_or("");
        if m.is_empty() { "static".to_string() } else { m.to_string() }
    };
    if mining_mode != "static" && mining_mode != "hybrid" {
        bail!("unsupported mining mode: {mining_mode}");
    }
    if is_running(bundle_dir) {
        bail!("a pipeline run is already in progress for this bundle");
    }

    // `start_code_bundle_build`'s real, grounded signature (pipeline.rs)
    // takes a `StartBuildOptions` struct, not positional args -- adapted at
    // this call site rather than changing the grounded signature.
    start_code_bundle_build(
        &repo_dir,
        bundle_dir,
        StartBuildOptions {
            mining_mode: Some(mining_mode.as_str()),
            render: form.get("render").map(|s| s.as_str()) == Some("on"),
            architecture: form.get("architecture").map(|s| s.as_str()) == Some("on"),
            ..StartBuildOptions::default()
        },
    )?;
    crate::model::invalidate_code_bundle_cache(bundle_dir);

    let mut out = Map::new();
    out.insert("status".into(), Value::String("started".into()));
    out.insert("repo_dir".into(), Value::String(repo_dir_str));
    out.insert("mining_mode".into(), Value::String(mining_mode));
    Ok(Value::Object(out))
}

/// Mirrors `_form_list`: read a repeated form field. The urlencoded parser
/// collapses repeats to the first value, so a multi-select arrives as one
/// newline-joined value from the page instead (matches `server.rs`'s
/// `parse_urlencoded_form_multi`, which joins repeats with `\n`).
fn form_list(form: &HashMap<String, String>, name: &str) -> Vec<String> {
    let raw = form.get(name).cloned().unwrap_or_default();
    raw.replace(',', "\n")
        .split('\n')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Mirrors `handle_code_review_form`: record one review decision, or the
/// same decision across many claims. A batch still writes one event per
/// claim, so the history stays per-claim and auditable.
///
/// Loads a fresh, uncached `CodeBundle` to get a mutable instance (Python
/// mutates the process-cached bundle object in place, relying on dict
/// aliasing; `model.rs`'s cache hands out an immutable `Arc<CodeBundle>`, so
/// this loads its own owned copy instead). What is *persisted* to
/// `code_reviews.json` -- the only thing that matters across requests -- is
/// identical either way.
pub fn handle_code_review_form(bundle_dir: &Path, form: &HashMap<String, String>) -> Result<Value> {
    let mut cb = CodeBundle::new(bundle_dir)?;
    let action = form.get("action").map(|s| s.as_str()).unwrap_or("comment").to_string();
    let reviewer = {
        let r = form.get("reviewer").map(|s| s.as_str()).unwrap_or("");
        if r.is_empty() { "local:user".to_string() } else { r.to_string() }
    };
    let rationale = form.get("rationale").cloned().unwrap_or_default();
    if rationale.trim().is_empty() {
        bail!("a rationale is required for a review decision");
    }

    let mut item_ids: Vec<String> =
        form_list(form, "item_ids").into_iter().filter(|i| cb.item_by_id.contains_key(i)).collect();
    if item_ids.is_empty() {
        let single = form.get("item_id").cloned().unwrap_or_default();
        if cb.item_by_id.contains_key(&single) {
            item_ids = vec![single];
        }
    }
    if item_ids.is_empty() {
        bail!("select at least one claim to review");
    }

    let mut seq = cb.review_events.len();
    let batch_size = item_ids.len();
    for item_id in &item_ids {
        let idx = *cb.item_by_id.get(item_id).expect("checked above");
        let prev = v_str(&cb.items[idx], "review_status");
        let new = review_actions(&action).map(str::to_string).unwrap_or_else(|| prev.clone());
        if review_actions(&action).is_some() {
            cb.review_status.insert(item_id.clone(), Value::String(new.clone()));
            if let Some(obj) = cb.items[idx].as_object_mut() {
                obj.insert("review_status".to_string(), Value::String(new.clone()));
            }
        }
        seq += 1;
        let mut event = Map::new();
        event.insert("id".into(), Value::String(format!("crev-{seq:04}")));
        event.insert("item_id".into(), Value::String(item_id.clone()));
        event.insert("action".into(), Value::String(action.clone()));
        event.insert("reviewer".into(), Value::String(reviewer.clone()));
        event.insert("rationale".into(), Value::String(rationale.clone()));
        event.insert("from".into(), Value::String(prev));
        event.insert("to".into(), Value::String(new));
        // Records that this decision was taken as part of a batch, so a
        // reviewer reading history later can tell it was not judged alone.
        event.insert("batch_size".into(), Value::from(batch_size));
        cb.review_events.push(Value::Object(event));
    }

    // Atomic: the workbench serves reads of this file while it is being
    // written.
    let mut payload = Map::new();
    payload.insert("status".into(), Value::Object(cb.review_status.clone()));
    payload.insert("events".into(), Value::Array(cb.review_events.clone()));
    crate::state::write_json(&cb.review_path(), &Value::Object(payload))?;
    crate::model::invalidate_code_bundle_cache(bundle_dir);

    let mut out = Map::new();
    out.insert("action".into(), Value::String(action));
    out.insert("count".into(), Value::from(item_ids.len()));
    Ok(Value::Object(out))
}

// ---------------------------------------------------------------------------
// Settings (py: lines 2481-2516)
// ---------------------------------------------------------------------------

/// Mirrors `render_settings_page`: read-only view of the LLM provider codekb
/// will use for hybrid mining.
///
/// WIRING: `crate::llm_settings` (Python's top-level `kl4a.llm_settings`) has
/// no Rust module or crate yet anywhere in this workspace.
pub fn render_settings_page() -> String {
    let status = if crate::llm_settings::is_configured() {
        "<p>LLM provider <span class=\"status ok\">configured</span>.</p>".to_string()
    } else {
        let missing = escape(&crate::llm_settings::missing_fields().join(", "));
        format!(
            "<p>LLM provider <span class=\"status warn\">not configured</span> \u{2014} missing {missing}.</p><p>A hybrid run still produces a valid static bundle; only the LLM enrichment step is skipped.</p>"
        )
    };
    let mut settings: Vec<(String, Value)> = crate::llm_settings::resolved_settings()
        .into_iter()
        .map(|(field, (value, source))| (field, json!({"value": value, "source": source})))
        .collect();
    settings.sort_by(|a, b| a.0.cmp(&b.0));
    let rows: String = settings
        .iter()
        .map(|(field, resolved)| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(field),
                escape(&crate::llm_settings::display_value(field)),
                escape(v_str_or(resolved, "source", ""))
            )
        })
        .collect();
    format!(
        "<h2>Settings</h2>\n<section class=\"panel\">\n  {status}\n  <table>\n    <thead><tr><th>Field</th><th>Value</th><th>Source</th></tr></thead>\n    <tbody>{rows}</tbody>\n  </table>\n  <p class=\"hint\">Secrets are masked. Change these in the SOP workbench's\n  Settings page - codekb reads the same saved settings file. An environment\n  variable or <code>.env</code> is only consulted for a field whose Source is\n  not already <code>saved</code>; neither overrides a saved value.</p>\n</section>"
    )
}
