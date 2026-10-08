//! Port of `kl4a/codekb/server.py`.
//!
//! Grounded via tools-code MCP: the Python module is a plain
//! `http.server.BaseHTTPRequestHandler` + `http.server.ThreadingHTTPServer`
//! (stdlib only — no framework), confirmed from `make_handler`'s and
//! `serve_bundle`'s bodies. This port follows the task instruction to match
//! that: manual HTTP request/response handling over `std::net::TcpListener`,
//! one OS thread per connection (mirroring `ThreadingHTTPServer`), no
//! framework crate and no async runtime.
//!
//! Python's `make_handler(bundle_dir)` dynamically builds a
//! `BaseHTTPRequestHandler` subclass that closes over `bundle_dir`; Rust has
//! no equivalent of building a class at runtime, so the same behavior is
//! reproduced by threading `bundle_dir` through plain functions
//! (`handle_connection`, `handle_get`, `handle_post`, `page`) instead of a
//! generated type. Every route in `do_GET`/`do_POST` is ported as its own
//! function below — none were skipped.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use anyhow::{bail, Result};
use once_cell::sync::Lazy;

use crate::model::{code_bundle_describe, is_code_bundle};

// WIRING: `kl4a/codekb/web.py` and `kl4a/codekb/layout.py` are not in this
// batch's scope (render.py/run_state.py/server.py/state.py only), so no Rust
// module exists yet under `v2/kl4a-rs/codekb/src/` for either. `layout` and
// `code_nav_items` below are grounded (their Python signatures were fetched
// via tools-code MCP: `layout(active: str, summary: dict, body: str,
// message: str, base_path: str = "", nav_views: list[str] |
// list[tuple[str, str]] | None = None, searchable: bool = False) -> str` and
// `code_nav_items(bundle_dir: Path) -> list[tuple[str, str]]`), so the Rust
// signatures used here follow those exactly. The rest
// (`render_code_page`, `render_settings_page`, `code_active_view`,
// `legacy_code_redirect`, `handle_code_pipeline_form`,
// `handle_code_review_form`) are inferred only from how `server.py` calls
// them — UNCONFIRMED pending whichever batch ports `web.py`. The coordinator
// should adjust either side to match once that port lands.
use crate::layout::{layout, NavView};
use crate::web::{
    code_active_view, code_nav_items, handle_code_pipeline_form, handle_code_review_form,
    legacy_code_redirect, render_code_page, render_settings_page,
};

/// Mirrors `kl4a.codekb.server.ACTION_RETURN_VIEWS`: POST action name ->
/// `(redirect_target, section, label)`.
static ACTION_RETURN_VIEWS: Lazy<HashMap<&'static str, (&'static str, &'static str, &'static str)>> =
    Lazy::new(|| {
        let mut m = HashMap::new();
        m.insert(
            "code-pipeline",
            ("overview?mode=pipeline", "overview", "Build"),
        );
        m.insert("review", ("knowledge", "knowledge", "Review"));
        m
    });

/// Mirrors `kl4a.codekb.server.parse_urlencoded_form`: keeps only the first
/// value per key (single-value form fields).
pub fn parse_urlencoded_form(payload: &[u8]) -> HashMap<String, String> {
    let text = String::from_utf8_lossy(payload);
    parse_qs(&text)
        .into_iter()
        .map(|(k, mut v)| (k, if v.is_empty() { String::new() } else { v.remove(0) }))
        .collect()
}

/// Mirrors `kl4a.codekb.server.parse_urlencoded_form_multi`: "Keep repeated
/// fields instead of discarding all but the first. A set of checkboxes
/// submits one repeated field; batch review depends on it." Repeated values
/// are joined with `\n`.
pub fn parse_urlencoded_form_multi(payload: &[u8]) -> HashMap<String, String> {
    let text = String::from_utf8_lossy(payload);
    parse_qs(&text)
        .into_iter()
        .map(|(k, v)| (k, v.join("\n")))
        .collect()
}

/// `urllib.parse.parse_qs` equivalent: `keep_blank_values=False` (the
/// Python default) drops blank values entirely, which this mirrors.
fn parse_qs(query: &str) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut it = pair.splitn(2, '=');
        let key = it.next().unwrap_or("");
        let value = it.next().unwrap_or("");
        if value.is_empty() {
            continue;
        }
        out.entry(percent_decode(key))
            .or_default()
            .push(percent_decode(value));
    }
    out
}

/// Percent-decodes a query/form component, treating `+` as space (the
/// `application/x-www-form-urlencoded` / `parse_qs` convention).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 3 <= bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    None => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `urllib.parse.quote(message, safe='')` equivalent: percent-encodes
/// everything except the unreserved set (letters, digits, `_.-~`).
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `html.escape` equivalent (default `quote=True`): order matters, `&` must
/// be escaped first.
fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Mirrors `make_handler(...).Handler._page`.
fn page(
    bundle_dir: &Path,
    view: &str,
    query: &HashMap<String, Vec<String>>,
    message: &str,
) -> Result<String> {
    let body = if view == "settings" {
        render_settings_page()
    } else {
        render_code_page(bundle_dir, view, query)?
    };
    let describe = code_bundle_describe(bundle_dir)?;
    // `code_nav_items` (web.rs) returns `Vec<(String, String)>` -- the literal
    // Rust shape of Python's `list[tuple[str, str]]` -- but `layout::layout`
    // (grounded independently against `layout.py`) takes `Option<&[NavView]>`.
    // Adapt at this call site rather than changing either grounded shape.
    let nav_views: Vec<NavView> = code_nav_items(bundle_dir)
        .into_iter()
        .map(|(route, label)| NavView::Pair(route, label))
        .collect();
    Ok(layout(
        &code_active_view(view),
        &describe,
        &body,
        message,
        "",
        Some(&nav_views),
        false,
    ))
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        303 => "See Other",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

/// Mirrors `Handler._send`.
fn send(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    let payload = body.as_bytes();
    write!(
        stream,
        "HTTP/1.1 {status} {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status_text(status),
        payload.len()
    )?;
    stream.write_all(payload)?;
    stream.flush()
}

/// Mirrors `Handler._redirect`.
fn redirect(stream: &mut TcpStream, location: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 303 See Other\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()
}

/// Mirrors `Handler.do_GET`.
fn handle_get(stream: &mut TcpStream, bundle_dir: &Path, raw_path: &str) {
    let (path_part, query_part) = raw_path.split_once('?').unwrap_or((raw_path, ""));
    let trimmed = path_part.trim_matches('/');
    let route = if trimmed.is_empty() { "overview" } else { trimmed };
    let mut query = parse_qs(query_part);

    let moved = if route == "settings" {
        None
    } else {
        legacy_code_redirect(route, &query)
    };
    if let Some(location) = moved {
        let _ = redirect(stream, &location);
        return;
    }

    let message = query
        .remove("message")
        .map(|mut v| if v.is_empty() { String::new() } else { v.remove(0) })
        .unwrap_or_default();

    // "a bad route should not kill the server" — every error is caught and
    // rendered, not allowed to tear down the connection handler.
    match page(bundle_dir, route, &query, &message) {
        Ok(body) => {
            let _ = send(stream, 200, &body);
        }
        Err(err) => {
            // Exception text routinely carries user-supplied values (bundle
            // and doc paths, query terms), so it is escaped like every other
            // value the workbench reflects back.
            let body = format!("<h2>Error</h2><pre>{}</pre>", escape_html(&format!("{err}")));
            let _ = send(stream, 500, &body);
        }
    }
}

/// Mirrors `Handler.do_POST`.
fn handle_post(stream: &mut TcpStream, bundle_dir: &Path, raw_path: &str, payload: &[u8]) {
    let action = raw_path.trim_matches('/');
    let Some(&(target, _section, label)) = ACTION_RETURN_VIEWS.get(action) else {
        let _ = send(stream, 404, "<h2>Unknown action</h2>");
        return;
    };

    let message = if action == "code-pipeline" {
        match handle_code_pipeline_form(bundle_dir, &parse_urlencoded_form(payload)) {
            Ok(_) => format!("{label} completed"),
            Err(err) => format!("{label} failed: {err}"),
        }
    } else {
        match handle_code_review_form(bundle_dir, &parse_urlencoded_form_multi(payload)) {
            Ok(_) => format!("{label} completed"),
            Err(err) => format!("{label} failed: {err}"),
        }
    };

    // Quoted: an "&" or "#" in the message would otherwise truncate it and a
    // newline would split the response header.
    let joiner = if target.contains('?') { "&" } else { "?" };
    let location = format!("/{target}{joiner}message={}", percent_encode(&message));
    let _ = redirect(stream, &location);
}

struct RequestHead {
    method: String,
    path: String,
    headers: HashMap<String, String>,
}

fn read_request_head(reader: &mut impl BufRead) -> std::io::Result<Option<RequestHead>> {
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(None);
    }
    let request_line = request_line.trim_end();
    if request_line.is_empty() {
        return Ok(None);
    }
    let mut parts = request_line.splitn(3, ' ');
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();

    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    Ok(Some(RequestHead { method, path, headers }))
}

/// One connection's worth of request handling. Stands in for the Python
/// `Handler` object `make_handler` builds: `bundle_dir` is threaded through
/// explicitly instead of captured by a dynamically generated class.
fn handle_connection(mut stream: TcpStream, bundle_dir: Arc<PathBuf>) {
    let peer = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut reader = BufReader::new(peer);
    let request = match read_request_head(&mut reader) {
        Ok(Some(r)) => r,
        _ => return,
    };

    match request.method.as_str() {
        "GET" => handle_get(&mut stream, &bundle_dir, &request.path),
        "POST" => {
            let length: usize = request
                .headers
                .get("content-length")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let mut body = vec![0u8; length];
            if reader.read_exact(&mut body).is_ok() {
                handle_post(&mut stream, &bundle_dir, &request.path, &body);
            }
        }
        _ => {
            let _ = send(&mut stream, 404, "<h2>Unknown action</h2>");
        }
    }
}

/// Mirrors `kl4a.codekb.server.serve_bundle`.
///
/// Resolves first: a relative path would otherwise be read against the
/// launch directory and present as an empty bundle rather than a wrong path.
/// UNCONFIRMED: Python's `Path.expanduser().resolve()` also expands a
/// leading `~` and resolves a path that does not yet exist; `std::fs::
/// canonicalize` does neither (no `~` handling, and it errors if the path is
/// missing), so a `~`-prefixed or not-yet-existing `bundle_dir` diverges from
/// Python here. Since `is_code_bundle` is checked immediately after and
/// requires the directory to already exist with bundle state in it, this gap
/// should not matter for any real caller.
pub fn serve_bundle(bundle_dir: &Path, host: &str, port: u16) -> Result<()> {
    let bundle_dir = bundle_dir
        .canonicalize()
        .unwrap_or_else(|_| bundle_dir.to_path_buf());
    if !is_code_bundle(&bundle_dir) {
        bail!(
            "Not a Code Knowledge Bundle: {}\nCreate one with: codekb init <bundle-dir> --repo <repo-dir>",
            crate::bundle_store::display_path(&bundle_dir)
        );
    }
    let listener = TcpListener::bind((host, port))?;
    println!("Serving Code Knowledge Bundle at http://{host}:{port}");
    println!("Bundle: {}", crate::bundle_store::display_path(&bundle_dir));

    let bundle_dir = Arc::new(bundle_dir);
    for incoming in listener.incoming() {
        let stream = match incoming {
            Ok(s) => s,
            Err(_) => continue,
        };
        let bundle_dir = Arc::clone(&bundle_dir);
        thread::spawn(move || handle_connection(stream, bundle_dir));
    }
    Ok(())
}
