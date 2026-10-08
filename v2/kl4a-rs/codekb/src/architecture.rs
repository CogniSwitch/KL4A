//! Port of `kl4a/codekb/architecture.py`.
//!
//! Static-analysis / architecture-detection: walks every Python source file
//! in the bundle once, collects FastAPI routers/apps, HTTP endpoints,
//! `include_router` mounts, and class definitions (classifying them into
//! SQLAlchemy ORM models vs. Pydantic/plain-data schemas), resolves each
//! endpoint's full mount path, categorizes third-party dependencies, folds
//! in whatever non-Python adapters contribute (COBOL today), and emits OKF
//! Markdown docs for all of it — then persists one `code_architecture.json`
//! state document.
//!
//! Ported symbol-for-symbol from the Python source (verified via tools-code
//! MCP `code_symbols_get`). Every helper `detect_architecture` calls is its
//! own function below, in the same call order the Python's own 16-step
//! procedure evidence lists — no helper collapsed into "the common case".
//!
//! ## Real Python-AST parsing via `rustpython_parser`
//!
//! The Python source parses every module with `ast.parse` and walks the
//! real Python AST (`ast.Import`, `ast.ClassDef`, `ast.Call`,
//! `ast.decorator_list`, `ast.unparse`, ...). This file now parses every
//! Python source with `rustpython_parser` and walks *that* real AST
//! (`rustpython_parser::ast::Stmt`/`Expr`) directly — the previous
//! regex/line-based scanner (marked `// UNCONFIRMED:` throughout) has been
//! replaced entirely. Every AST-walking helper (`_collect_imports`,
//! `_collect_assignments`, `_collect_endpoints`, `_collect_includes`,
//! `_collect_classes`, and the small `ast.Call` accessors
//! `_kwarg_str`/`_kwarg_list`/`_first_arg_type`/`_decorator_bare_name`/
//! `_decorator_target`) was re-grounded against its exact Python source via
//! tools-code MCP (`code_symbols_get`) and rewritten against the real parsed
//! tree; see the coordinator handback report for the per-function ledger.
//!
//! `_kwarg_str`/`_kwarg_list` now stringify *any* `ast.Constant` value
//! (matching Python's `str(kw.value.value)`, not just string constants —
//! the old regex scan's `as_python_constant_str` only handled the common
//! cases). `_first_arg_type`/base-class/annotation text now come from a
//! real recursive `ast.unparse` reconstruction built directly off the
//! parsed tree's structured fields (see `unparse_expr` below), not a
//! verbatim source-text slice.
//!
//! ## Cross-batch dependencies (NOT part of this batch — report only)
//!
//! - `kl4a.codekb.state.{STATE_DIR, read_json, write_json}` (source file
//!   `kl4a/codekb/state.py`) — expected as `crate::state::{STATE_DIR,
//!   read_json, write_json}`. Python's own `_load_json` in this file is a
//!   thin one-off (`json.loads(path.read_text()) if path.exists() else {}`)
//!   distinct from the shared `state.read_json` (which additionally
//!   tolerates a torn/empty file) — ported faithfully as its own function,
//!   not merged with the shared one, per "port every mechanism, not your
//!   read of which one is close enough."
//! - `kl4a.codekb.parse.module_name_for` (source file `kl4a/codekb/parse.py`)
//!   — expected as `crate::parse::module_name_for`.
//! - `kl4a.codekb.entrypoints.collect_entrypoints` (source file
//!   `kl4a/codekb/entrypoints.py`) — expected as
//!   `crate::entrypoints::collect_entrypoints`, returning
//!   `(extra_endpoints, extra_frameworks, flask_vars)`. **This is where the
//!   non-FastAPI framework detection actually lives** (Flask, Django,
//!   Click/argparse, Celery per the Python's own comment
//!   `# Non-FastAPI entrypoints (Flask, Django, Click/argparse, Celery) in
//!   the same schema.`) — `architecture.py` itself has no per-framework
//!   detector branches beyond the generic FastAPI/SQLAlchemy/Pydantic scan
//!   below; it delegates the rest wholesale to this one call.
//! - `kl4a.codekb.adapters.cobol_architecture.detect_cobol_architecture`
//!   (source file `kl4a/codekb/adapters/cobol_architecture.py`) — expected
//!   as `crate::adapters::cobol_architecture::detect_cobol_architecture`.
//! - `kl4a.kl4a.okf_writer.write_markdown` (source file
//!   `kl4a/kl4a/okf_writer.py`, shared package) — expected as
//!   `crate::okf_writer::write_markdown`.
//! - `kl4a.codekb.state.write_code_state` (referenced by
//!   `detect_architecture`'s last line, `write_code_state(bundle_dir,
//!   "code_architecture.json", state)`) — expected as
//!   `crate::state::write_code_state`.
//!
//! None of these modules exist yet in `v2/kl4a-rs`; this file compiles once
//! whichever batches port them add the matching `pub mod` to `codekb`'s
//! `lib.rs`. See the handback report for the exact ledger entries.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use once_cell::sync::Lazy;
use rustpython_parser::ast::{self as pyast_ast, Ranged};
use rustpython_parser::Mode;
use serde_json::{json, Value};

use crate::ids::{bounded_id, code_symbol_id_for};

// ---------------------------------------------------------------------
// Byte-offset -> 1-based line number (rustpython_parser's `TextRange`s are
// byte offsets, not pre-computed line numbers). Vendored independently in
// `parse.rs` and `entrypoints.rs` too — each file's own small copy, per this
// codebase's established "vendor and reconcile" convention for small
// cross-batch helpers (see `entrypoints.rs`'s own note on `_kwarg_str` etc.).
struct LineIndex {
    starts: Vec<usize>,
}
impl LineIndex {
    fn new(source: &str) -> Self {
        let mut starts = vec![0usize];
        for (i, c) in source.char_indices() {
            if c == '\n' {
                starts.push(i + 1);
            }
        }
        LineIndex { starts }
    }
    fn at(&self, byte_offset: usize) -> usize {
        match self.starts.binary_search(&byte_offset) {
            Ok(i) => i + 1,
            Err(idx) => idx.max(1),
        }
    }
}
fn start_line<T: Ranged>(li: &LineIndex, node: &T) -> usize {
    li.at(u32::from(node.range().start()) as usize)
}

/// Port of `kl4a.codekb.architecture.HTTP_METHODS`.
pub static HTTP_METHODS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    ["get", "post", "put", "delete", "patch", "options", "head", "trace"]
        .into_iter()
        .collect()
});

/// Port of `kl4a.codekb.architecture.REPO_ROOTS_DEFAULT`.
pub static REPO_ROOTS_DEFAULT: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["app", "scripts", "tests", "main"].into_iter().collect());

/// Port of `kl4a.codekb.architecture.DEP_CATEGORIES`, verbatim (key order as
/// in the Python dict; iteration order does not matter here since it is
/// only ever looked up by key via `.get`).
pub static DEP_CATEGORIES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    [
        ("fastapi", "Web framework"),
        ("starlette", "Web framework"),
        ("uvicorn", "ASGI server"),
        ("pydantic", "Validation/serialization"),
        ("sqlalchemy", "Database ORM"),
        ("psycopg2", "PostgreSQL driver"),
        ("asyncpg", "PostgreSQL driver"),
        ("snowflake", "Data warehouse (Snowflake)"),
        ("boto3", "AWS SDK"),
        ("botocore", "AWS SDK"),
        ("s3transfer", "AWS SDK"),
        ("httpx", "HTTP client"),
        ("requests", "HTTP client"),
        ("aiohttp", "HTTP client"),
        ("pandas", "Data processing"),
        ("numpy", "Data processing"),
        ("openpyxl", "Spreadsheet I/O"),
        ("jwt", "Auth/crypto (PyJWT)"),
        ("cryptography", "Auth/crypto"),
        ("OpenSSL", "Auth/crypto (pyOpenSSL)"),
        ("dotenv", "Config (python-dotenv)"),
        ("redis", "Cache"),
        ("celery", "Task queue"),
        ("pytest", "Testing"),
        ("click", "CLI framework"),
        ("argparse", "CLI framework"),
        ("flask", "Web framework"),
        ("django", "Web framework"),
        ("yaml", "Serialization (YAML)"),
        ("docx", "Document I/O (python-docx)"),
        ("pdfplumber", "Document I/O (PDF)"),
        ("openai", "LLM client"),
        ("anthropic", "LLM client"),
    ]
    .into_iter()
    .collect()
});

/// Port of `kl4a.codekb.architecture.ARCH_DIRS`.
pub const ARCH_DIRS: [&str; 4] = [
    "code/endpoints",
    "code/data-models",
    "code/schemas",
    "code/dependencies",
];

/// Port of `kl4a.codekb.architecture.PLAIN_DATA_DECORATORS`.
pub static PLAIN_DATA_DECORATORS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["dataclass", "define", "frozen"].into_iter().collect());

/// Port of `kl4a.codekb.architecture.PLAIN_DATA_BASES`.
pub static PLAIN_DATA_BASES: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["NamedTuple", "TypedDict"].into_iter().collect());

// ---------------------------------------------------------------------
// Real `ast.Call`/`ast.unparse` accessors, against `rustpython_parser`'s AST.
// ---------------------------------------------------------------------

/// A minimal, best-effort `ast.unparse` for the expression shapes that
/// appear as class bases, annotations, and `Column(...)`/`mapped_column(...)`
/// first-argument types in this codebase — built by recursing over the real
/// parsed tree rather than slicing source text, so it normalizes formatting
/// (spacing, quoting) the way `ast.unparse` does. Not a complete Python
/// unparser (e.g. lambda bodies and slices are approximated), but every
/// shape this file's own callers (`_collect_classes`'s bases/annotations,
/// `_first_arg_type`) actually need is covered.
fn unparse_expr(e: &pyast_ast::Expr) -> String {
    use pyast_ast::Expr;
    match e {
        Expr::Name(n) => n.id.to_string(),
        Expr::Attribute(a) => format!("{}.{}", unparse_expr(&a.value), a.attr.as_str()),
        Expr::Call(c) => {
            let args: Vec<String> = c
                .args
                .iter()
                .map(unparse_expr)
                .chain(c.keywords.iter().map(|k| match &k.arg {
                    Some(name) => format!("{}={}", name.as_str(), unparse_expr(&k.value)),
                    None => format!("**{}", unparse_expr(&k.value)),
                }))
                .collect();
            format!("{}({})", unparse_expr(&c.func), args.join(", "))
        }
        Expr::Constant(c) => unparse_constant(&c.value),
        Expr::Subscript(s) => {
            // Bug confirmed via a full `tools/kl4a` self-build diff: a
            // Pydantic/dataclass schema field annotated `dict[str, Any]`
            // rendered here as `dict[(str, Any)]` — `ast.unparse` omits the
            // outer parens around a tuple subscript slice (same fidelity
            // gap already fixed for `parse.rs`'s own, separate unparser;
            // this file has its own independent `unparse_expr` that didn't
            // get that fix applied, since it doesn't share code with
            // `parse.rs::pyast`). The single-element trailing comma
            // (`Tuple[int,]`) is kept since it's syntactically meaningful.
            let slice_text = match s.slice.as_ref() {
                Expr::Tuple(t) => {
                    let joined = t.elts.iter().map(unparse_expr).collect::<Vec<_>>().join(", ");
                    if t.elts.len() == 1 { format!("{joined},") } else { joined }
                }
                other => unparse_expr(other),
            };
            format!("{}[{}]", unparse_expr(&s.value), slice_text)
        }
        Expr::BinOp(b) => format!("{} {} {}", unparse_expr(&b.left), binop_symbol(&b.op), unparse_expr(&b.right)),
        Expr::UnaryOp(u) => {
            let sym = match u.op {
                pyast_ast::UnaryOp::Invert => "~",
                pyast_ast::UnaryOp::Not => "not ",
                pyast_ast::UnaryOp::UAdd => "+",
                pyast_ast::UnaryOp::USub => "-",
            };
            format!("{sym}{}", unparse_expr(&u.operand))
        }
        Expr::List(l) => format!("[{}]", l.elts.iter().map(unparse_expr).collect::<Vec<_>>().join(", ")),
        Expr::Tuple(t) => {
            let joined = t.elts.iter().map(unparse_expr).collect::<Vec<_>>().join(", ");
            if t.elts.len() == 1 { format!("({joined},)") } else { format!("({joined})") }
        }
        Expr::Starred(s) => format!("*{}", unparse_expr(&s.value)),
        _ => "<expr>".to_string(),
    }
}

fn binop_symbol(op: &pyast_ast::Operator) -> &'static str {
    match op {
        pyast_ast::Operator::Add => "+",
        pyast_ast::Operator::Sub => "-",
        pyast_ast::Operator::Mult => "*",
        pyast_ast::Operator::MatMult => "@",
        pyast_ast::Operator::Div => "/",
        pyast_ast::Operator::Mod => "%",
        pyast_ast::Operator::Pow => "**",
        pyast_ast::Operator::LShift => "<<",
        pyast_ast::Operator::RShift => ">>",
        pyast_ast::Operator::BitOr => "|",
        pyast_ast::Operator::BitXor => "^",
        pyast_ast::Operator::BitAnd => "&",
        pyast_ast::Operator::FloorDiv => "//",
    }
}

/// Stringifies any `ast.Constant.value` the way Python's `str(...)` would —
/// used by [`kwarg_str`]/[`kwarg_list`], which (per the grounded Python
/// source) stringify *any* constant keyword value, not just strings.
fn stringify_constant(c: &pyast_ast::Constant) -> String {
    match c {
        pyast_ast::Constant::None => "None".to_string(),
        pyast_ast::Constant::Bool(b) => if *b { "True".to_string() } else { "False".to_string() },
        pyast_ast::Constant::Str(s) => s.clone(),
        pyast_ast::Constant::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
        pyast_ast::Constant::Int(i) => i.to_string(),
        pyast_ast::Constant::Float(f) => f.to_string(),
        pyast_ast::Constant::Complex { real, imag } => format!("({real}+{imag}j)"),
        pyast_ast::Constant::Tuple(items) => {
            format!("({})", items.iter().map(stringify_constant).collect::<Vec<_>>().join(", "))
        }
        pyast_ast::Constant::Ellipsis => "Ellipsis".to_string(),
    }
}

/// `ast.unparse`-style `repr()` quoting for a constant, used by
/// [`unparse_expr`] (a base-class/annotation/first-arg-type expression that
/// happens to be a literal, e.g. `ForeignKey("users.id")`, must round-trip
/// as quoted Python source, unlike `stringify_constant`'s bare `str()`).
fn unparse_constant(c: &pyast_ast::Constant) -> String {
    match c {
        pyast_ast::Constant::Str(s) => {
            let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
            let mut out = String::with_capacity(s.len() + 2);
            out.push(quote);
            for ch in s.chars() {
                match ch {
                    '\\' => out.push_str("\\\\"),
                    c if c == quote => { out.push('\\'); out.push(c); }
                    c => out.push(c),
                }
            }
            out.push(quote);
            out
        }
        other => stringify_constant(other),
    }
}

/// Port of `kl4a.codekb.architecture._kwarg_str`.
///
/// Grounded via MCP against the exact Python source
/// (`for kw in call.keywords: if kw.arg == name and
/// isinstance(kw.value, ast.Constant): return str(kw.value.value)`):
/// stringifies *any* constant, not just a string one.
fn kwarg_str(keywords: &[pyast_ast::Keyword], name: &str) -> Option<String> {
    keywords.iter().find(|kw| kw.arg.as_deref() == Some(name)).and_then(|kw| match &kw.value {
        pyast_ast::Expr::Constant(c) => Some(stringify_constant(&c.value)),
        _ => None,
    })
}

/// Port of `kl4a.codekb.architecture._kwarg_list`.
///
/// Grounded via MCP: `for kw in call.keywords: if kw.arg == name and
/// isinstance(kw.value, (ast.List, ast.Tuple)): return [str(e.value) for e
/// in kw.value.elts if isinstance(e, ast.Constant)]`.
fn kwarg_list(keywords: &[pyast_ast::Keyword], name: &str) -> Vec<String> {
    let Some(kw) = keywords.iter().find(|kw| kw.arg.as_deref() == Some(name)) else {
        return Vec::new();
    };
    let elts: &[pyast_ast::Expr] = match &kw.value {
        pyast_ast::Expr::List(l) => &l.elts,
        pyast_ast::Expr::Tuple(t) => &t.elts,
        _ => return Vec::new(),
    };
    elts.iter()
        .filter_map(|e| match e {
            pyast_ast::Expr::Constant(c) => Some(stringify_constant(&c.value)),
            _ => None,
        })
        .collect()
}

/// Port of `kl4a.codekb.architecture._first_arg_type`.
///
/// Grounded via MCP: `if call.args: try: return ast.unparse(call.args[0])
/// except Exception: return None`. The real parsed tree always yields a
/// valid expression for an already-parsed `call.args[0]`, so the `except`
/// branch (defensive in Python against exotic unparse failures) has no
/// analog here.
fn first_arg_type(call: &pyast_ast::ExprCall) -> Option<String> {
    call.args.first().map(unparse_expr)
}

/// Port of `kl4a.codekb.architecture._decorator_bare_name`.
///
/// Grounded via MCP: `target = dec.func if isinstance(dec, ast.Call) else
/// dec; return target.id if isinstance(target, ast.Name) else (target.attr
/// if isinstance(target, ast.Attribute) else None)`.
fn decorator_bare_name(dec: &pyast_ast::Expr) -> Option<String> {
    let target = match dec {
        pyast_ast::Expr::Call(c) => c.func.as_ref(),
        other => other,
    };
    match target {
        pyast_ast::Expr::Name(n) => Some(n.id.to_string()),
        pyast_ast::Expr::Attribute(a) => Some(a.attr.to_string()),
        _ => None,
    }
}

/// Port of `kl4a.codekb.architecture._decorator_target`.
///
/// Grounded via MCP: `call = dec if isinstance(dec, ast.Call) else None;
/// attr_node = call.func if call else dec; if isinstance(attr_node,
/// ast.Attribute) and isinstance(attr_node.value, ast.Name): return
/// attr_node.value.id, attr_node.attr, call; return None, None, None`.
/// Note Python returns a non-`None` `(var, attr)` pair even when `dec` is a
/// bare `@router.get` with no call — callers must additionally check the
/// returned `call` is `Some` (mirrored here as the third tuple slot being
/// `Option`, not the whole return being `None`).
fn decorator_target(dec: &pyast_ast::Expr) -> Option<(String, String, Option<&pyast_ast::ExprCall>)> {
    let call = match dec {
        pyast_ast::Expr::Call(c) => Some(c),
        _ => None,
    };
    let attr_node = match call {
        Some(c) => c.func.as_ref(),
        None => dec,
    };
    match attr_node {
        pyast_ast::Expr::Attribute(a) => match a.value.as_ref() {
            pyast_ast::Expr::Name(n) => Some((n.id.to_string(), a.attr.to_string(), call)),
            _ => None,
        },
        _ => None,
    }
}

/// Port of `kl4a.codekb.architecture._is_plain_data_class`.
///
/// A class describing a data shape without any web-framework/ORM base --
/// `@dataclass`, attrs' `@define`/`@frozen`, or a `NamedTuple`/`TypedDict`
/// base. Without this, `schemas`/`data_models` are hard-zero for any
/// framework-free codebase, even one full of real, meaningful data classes.
fn is_plain_data_class(decorator_names: &BTreeSet<String>, base_names: &BTreeSet<String>) -> bool {
    decorator_names
        .iter()
        .any(|d| PLAIN_DATA_DECORATORS.contains(d.as_str()))
        || base_names
            .iter()
            .any(|b| PLAIN_DATA_BASES.contains(b.as_str()))
}

/// Port of `kl4a.codekb.architecture._join_path`.
fn join_path(parts: &[&str]) -> String {
    let segs: Vec<String> = parts
        .iter()
        .filter(|p| !p.is_empty())
        .map(|p| format!("/{}", p.trim_matches('/')))
        .collect();
    segs.concat()
}

/// Port of `kl4a.codekb.architecture._load_json`.
///
/// A thin one-off (not the shared `crate::state::read_json`) — see the
/// module-level cross-batch note for why the two are kept distinct.
fn load_json(path: &Path) -> Value {
    if !path.exists() {
        return json!({});
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}))
}

/// Port of `kl4a.codekb.architecture._maybe_symbol`.
fn maybe_symbol(qname: &str) -> String {
    code_symbol_id_for(qname)
}

/// Port of `kl4a.codekb.architecture._reset_dir`.
fn reset_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    for entry in std::fs::read_dir(path)?.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("md")
            && p.file_name().and_then(|n| n.to_str()) != Some("index.md")
        {
            std::fs::remove_file(&p)?;
        }
    }
    Ok(())
}

/// Port of `kl4a.codekb.architecture._endpoint_title`.
fn endpoint_title(e: &Value) -> String {
    let kind = e.get("kind").and_then(Value::as_str).unwrap_or("http");
    let name_or_path = || {
        e.get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| e.get("path").and_then(Value::as_str).unwrap_or("").to_string())
    };
    match kind {
        "cli" => format!("CLI: {}", name_or_path()),
        "task" => format!("Task: {}", name_or_path()),
        _ => format!(
            "{} {}",
            e.get("method").and_then(Value::as_str).unwrap_or(""),
            e.get("path").and_then(Value::as_str).unwrap_or(""),
        ),
    }
}

// ---------------------------------------------------------------------
// AST-walking collectors, against `rustpython_parser`'s real parsed tree.
// ---------------------------------------------------------------------

/// Every statement reachable from `stmts`, at any depth (mirrors
/// `ast.walk(tree)` restricted to statement nodes — imports/`Call`s can
/// never appear as bare expressions the way a `Call` can be nested inside
/// another expression, so a statement-level walk is sufficient for
/// [`collect_imports`]; [`collect_includes`] additionally needs every
/// expression, via [`all_exprs`]).
fn all_stmts<'a>(stmts: &'a [pyast_ast::Stmt], out: &mut Vec<&'a pyast_ast::Stmt>) {
    for s in stmts {
        out.push(s);
        match s {
            pyast_ast::Stmt::FunctionDef(f) => all_stmts(&f.body, out),
            pyast_ast::Stmt::AsyncFunctionDef(f) => all_stmts(&f.body, out),
            pyast_ast::Stmt::ClassDef(c) => all_stmts(&c.body, out),
            pyast_ast::Stmt::If(i) => {
                all_stmts(&i.body, out);
                all_stmts(&i.orelse, out);
            }
            pyast_ast::Stmt::For(f) => {
                all_stmts(&f.body, out);
                all_stmts(&f.orelse, out);
            }
            pyast_ast::Stmt::AsyncFor(f) => {
                all_stmts(&f.body, out);
                all_stmts(&f.orelse, out);
            }
            pyast_ast::Stmt::While(w) => {
                all_stmts(&w.body, out);
                all_stmts(&w.orelse, out);
            }
            pyast_ast::Stmt::With(w) => all_stmts(&w.body, out),
            pyast_ast::Stmt::AsyncWith(w) => all_stmts(&w.body, out),
            pyast_ast::Stmt::Try(t) => {
                all_stmts(&t.body, out);
                for h in &t.handlers {
                    let pyast_ast::ExceptHandler::ExceptHandler(eh) = h;
                    all_stmts(&eh.body, out);
                }
                all_stmts(&t.orelse, out);
                all_stmts(&t.finalbody, out);
            }
            pyast_ast::Stmt::TryStar(t) => {
                all_stmts(&t.body, out);
                for h in &t.handlers {
                    let pyast_ast::ExceptHandler::ExceptHandler(eh) = h;
                    all_stmts(&eh.body, out);
                }
                all_stmts(&t.orelse, out);
                all_stmts(&t.finalbody, out);
            }
            pyast_ast::Stmt::Match(m) => {
                for case in &m.cases {
                    all_stmts(&case.body, out);
                }
            }
            _ => {}
        }
    }
}

/// The direct (non-recursing) `Expr` fields of a single statement, for
/// [`all_exprs`] to walk into.
fn direct_exprs(stmt: &pyast_ast::Stmt) -> Vec<&pyast_ast::Expr> {
    use pyast_ast::Stmt;
    match stmt {
        Stmt::Assign(a) => a.targets.iter().chain(std::iter::once(a.value.as_ref())).collect(),
        Stmt::Expr(e) => vec![&e.value],
        Stmt::If(i) => vec![&i.test],
        Stmt::For(f) => vec![&f.target, &f.iter],
        Stmt::AsyncFor(f) => vec![&f.target, &f.iter],
        Stmt::While(w) => vec![&w.test],
        Stmt::With(w) => w.items.iter().map(|it| &it.context_expr).collect(),
        Stmt::AsyncWith(w) => w.items.iter().map(|it| &it.context_expr).collect(),
        Stmt::Return(r) => r.value.iter().map(|v| v.as_ref()).collect(),
        Stmt::Raise(r) => r.exc.iter().chain(r.cause.iter()).map(|v| v.as_ref()).collect(),
        Stmt::Assert(a) => a.msg.iter().map(|v| v.as_ref()).chain(std::iter::once(&*a.test)).collect(),
        Stmt::FunctionDef(f) => f.decorator_list.iter().collect(),
        Stmt::AsyncFunctionDef(f) => f.decorator_list.iter().collect(),
        Stmt::ClassDef(c) => c.decorator_list.iter().chain(c.bases.iter()).collect(),
        _ => vec![],
    }
}

/// Recursively descends into every sub-expression of `expr` (call
/// args/keywords, attribute values, subscripts, ...) — the expression-side
/// half of an `ast.walk(tree)` equivalent.
fn walk_expr_tree<'a>(expr: &'a pyast_ast::Expr, out: &mut Vec<&'a pyast_ast::Expr>) {
    use pyast_ast::Expr;
    out.push(expr);
    match expr {
        Expr::Attribute(a) => walk_expr_tree(&a.value, out),
        Expr::Call(c) => {
            walk_expr_tree(&c.func, out);
            for a in &c.args {
                walk_expr_tree(a, out);
            }
            for k in &c.keywords {
                walk_expr_tree(&k.value, out);
            }
        }
        Expr::BinOp(b) => {
            walk_expr_tree(&b.left, out);
            walk_expr_tree(&b.right, out);
        }
        Expr::BoolOp(b) => {
            for v in &b.values {
                walk_expr_tree(v, out);
            }
        }
        Expr::UnaryOp(u) => walk_expr_tree(&u.operand, out),
        Expr::Compare(c) => {
            walk_expr_tree(&c.left, out);
            for comp in &c.comparators {
                walk_expr_tree(comp, out);
            }
        }
        Expr::List(l) => {
            for e in &l.elts {
                walk_expr_tree(e, out);
            }
        }
        Expr::Tuple(t) => {
            for e in &t.elts {
                walk_expr_tree(e, out);
            }
        }
        Expr::Set(s) => {
            for e in &s.elts {
                walk_expr_tree(e, out);
            }
        }
        Expr::Dict(d) => {
            for k in d.keys.iter().flatten() {
                walk_expr_tree(k, out);
            }
            for v in &d.values {
                walk_expr_tree(v, out);
            }
        }
        Expr::Subscript(s) => {
            walk_expr_tree(&s.value, out);
            walk_expr_tree(&s.slice, out);
        }
        Expr::Starred(s) => walk_expr_tree(&s.value, out),
        Expr::IfExp(i) => {
            walk_expr_tree(&i.body, out);
            walk_expr_tree(&i.test, out);
            walk_expr_tree(&i.orelse, out);
        }
        Expr::Await(a) => walk_expr_tree(&a.value, out),
        Expr::Yield(y) => {
            if let Some(v) = &y.value {
                walk_expr_tree(v, out);
            }
        }
        Expr::YieldFrom(y) => walk_expr_tree(&y.value, out),
        Expr::NamedExpr(n) => {
            walk_expr_tree(&n.target, out);
            walk_expr_tree(&n.value, out);
        }
        _ => {}
    }
}

/// Every `Expr` node reachable anywhere in `stmts` (combines [`all_stmts`]
/// with [`walk_expr_tree`]) — the full `Expr`+`Stmt` equivalent of
/// `ast.walk(tree)` for the node kinds this file matches on.
fn all_exprs(stmts: &[pyast_ast::Stmt]) -> Vec<&pyast_ast::Expr> {
    let mut top_stmts = Vec::new();
    all_stmts(stmts, &mut top_stmts);
    let mut out = Vec::new();
    for stmt in top_stmts {
        for e in direct_exprs(stmt) {
            walk_expr_tree(e, &mut out);
        }
    }
    out
}

/// Port of `kl4a.codekb.architecture._collect_imports`.
///
/// Grounded via MCP: Python walks the *whole* AST (`ast.walk(tree)`, i.e.
/// every import anywhere, including inside functions/conditionals), and
/// only follows an `ast.ImportFrom` when `node.level == 0` (an absolute
/// import — `from . import x` / `from .. import y` are skipped).
fn collect_imports(
    module_ast: &pyast_ast::ModModule,
    module: &str,
    path: &str,
    repo_roots: &HashSet<String>,
    dep_hits: &mut BTreeMap<String, Value>,
    frameworks: &mut BTreeSet<String>,
) {
    let mut stmts = Vec::new();
    all_stmts(&module_ast.body, &mut stmts);
    for stmt in stmts {
        let roots: Vec<String> = match stmt {
            pyast_ast::Stmt::Import(imp) => imp
                .names
                .iter()
                .map(|alias| alias.name.split('.').next().unwrap_or_default().to_string())
                .collect(),
            pyast_ast::Stmt::ImportFrom(imp) => {
                let level_is_zero = imp.level.map(|l| l.to_u32() == 0).unwrap_or(true);
                match (&imp.module, level_is_zero) {
                    (Some(m), true) => vec![m.split('.').next().unwrap_or_default().to_string()],
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        };
        for root in roots {
            if root.is_empty() || repo_roots.contains(&root) || is_stdlib_module(&root) {
                continue;
            }
            if root == "fastapi" {
                frameworks.insert("fastapi".to_string());
            }
            if root == "pydantic" {
                frameworks.insert("pydantic".to_string());
            }
            if root == "sqlalchemy" {
                frameworks.insert("sqlalchemy".to_string());
            }
            let hit = dep_hits.entry(root.clone()).or_insert_with(|| {
                json!({
                    "id": bounded_id("dependency", &root, 96),
                    "package": root,
                    "category": DEP_CATEGORIES.get(root.as_str()).copied().unwrap_or("External library"),
                    "modules": [],
                })
            });
            if let Some(modules) = hit.get_mut("modules").and_then(Value::as_array_mut) {
                let module_val = json!(module);
                if !modules.contains(&module_val) {
                    modules.push(module_val);
                }
            }
        }
    }
    let _ = path;
}

/// Fix (Medium finding): the full `sys.stdlib_module_names` list, not a
/// ~60-name hand-maintained allowlist. The previous version here covered
/// only the stdlib modules an auditor happened to think of, which both
/// false-classified real stdlib modules (e.g. `graphlib`, `zoneinfo`-
/// adjacent modules, `tomllib`, `secrets`, ...) as third-party dependencies
/// *and* produced the specific spurious-dependency bug the audit flagged:
/// `__future__` was missing from the old list, so `from __future__ import
/// annotations` (present in most files in this very codebase, e.g.
/// `tools/kl4a/codekb/parse.py` line 1) was misclassified as an external
/// package dependency named `__future__`.
///
/// Captured from `sys.stdlib_module_names` on the Python interpreter this
/// repo's own reference implementation runs on
/// (`.venv/Scripts/python.exe`, CPython 3.13.7 on Windows — confirmed via
/// `python -c "import sys,json; print(sys.version); print(json.dumps(sorted(sys.stdlib_module_names)))"`,
/// 290 names). `sys.stdlib_module_names` is itself interpreter-version-
/// dependent and has no Rust equivalent to query at either compile or run
/// time, so this list is captured data, not computed — if codekb is later
/// built against a different Python version's semantics, this list should
/// be recaptured the same way. Includes platform-specific/private
/// (`_`-prefixed) and build-only modules exactly as CPython itself lists
/// them, since Python's own check (`root in sys.stdlib_module_names`) does
/// not filter those out either.
fn is_stdlib_module(root: &str) -> bool {
    const STDLIB: &[&str] = &[
        "__future__", "_abc", "_aix_support", "_android_support", "_apple_support", "_ast",
        "_asyncio", "_bisect", "_blake2", "_bz2", "_codecs", "_codecs_cn", "_codecs_hk",
        "_codecs_iso2022", "_codecs_jp", "_codecs_kr", "_codecs_tw", "_collections",
        "_collections_abc", "_colorize", "_compat_pickle", "_compression", "_contextvars",
        "_csv", "_ctypes", "_curses", "_curses_panel", "_datetime", "_dbm", "_decimal",
        "_elementtree", "_frozen_importlib", "_frozen_importlib_external", "_functools",
        "_gdbm", "_hashlib", "_heapq", "_imp", "_interpchannels", "_interpqueues",
        "_interpreters", "_io", "_ios_support", "_json", "_locale", "_lsprof", "_lzma",
        "_markupbase", "_md5", "_multibytecodec", "_multiprocessing", "_opcode",
        "_opcode_metadata", "_operator", "_osx_support", "_overlapped", "_pickle",
        "_posixshmem", "_posixsubprocess", "_py_abc", "_pydatetime", "_pydecimal", "_pyio",
        "_pylong", "_pyrepl", "_queue", "_random", "_scproxy", "_sha1", "_sha2", "_sha3",
        "_signal", "_sitebuiltins", "_socket", "_sqlite3", "_sre", "_ssl", "_stat",
        "_statistics", "_string", "_strptime", "_struct", "_suggestions", "_symtable",
        "_sysconfig", "_thread", "_threading_local", "_tkinter", "_tokenize", "_tracemalloc",
        "_typing", "_uuid", "_warnings", "_weakref", "_weakrefset", "_winapi", "_wmi",
        "_zoneinfo", "abc", "antigravity", "argparse", "array", "ast", "asyncio", "atexit",
        "base64", "bdb", "binascii", "bisect", "builtins", "bz2", "cProfile", "calendar",
        "cmath", "cmd", "code", "codecs", "codeop", "collections", "colorsys", "compileall",
        "concurrent", "configparser", "contextlib", "contextvars", "copy", "copyreg", "csv",
        "ctypes", "curses", "dataclasses", "datetime", "dbm", "decimal", "difflib", "dis",
        "doctest", "email", "encodings", "ensurepip", "enum", "errno", "faulthandler", "fcntl",
        "filecmp", "fileinput", "fnmatch", "fractions", "ftplib", "functools", "gc",
        "genericpath", "getopt", "getpass", "gettext", "glob", "graphlib", "grp", "gzip",
        "hashlib", "heapq", "hmac", "html", "http", "idlelib", "imaplib", "importlib",
        "inspect", "io", "ipaddress", "itertools", "json", "keyword", "linecache", "locale",
        "logging", "lzma", "mailbox", "marshal", "math", "mimetypes", "mmap", "modulefinder",
        "msvcrt", "multiprocessing", "netrc", "nt", "ntpath", "nturl2path", "numbers", "opcode",
        "operator", "optparse", "os", "pathlib", "pdb", "pickle", "pickletools", "pkgutil",
        "platform", "plistlib", "poplib", "posix", "posixpath", "pprint", "profile", "pstats",
        "pty", "pwd", "py_compile", "pyclbr", "pydoc", "pydoc_data", "pyexpat", "queue",
        "quopri", "random", "re", "readline", "reprlib", "resource", "rlcompleter", "runpy",
        "sched", "secrets", "select", "selectors", "shelve", "shlex", "shutil", "signal",
        "site", "smtplib", "socket", "socketserver", "sqlite3", "sre_compile", "sre_constants",
        "sre_parse", "ssl", "stat", "statistics", "string", "stringprep", "struct",
        "subprocess", "symtable", "sys", "sysconfig", "syslog", "tabnanny", "tarfile",
        "tempfile", "termios", "textwrap", "this", "threading", "time", "timeit", "tkinter",
        "token", "tokenize", "tomllib", "trace", "traceback", "tracemalloc", "tty", "turtle",
        "turtledemo", "types", "typing", "unicodedata", "unittest", "urllib", "uuid", "venv",
        "warnings", "wave", "weakref", "webbrowser", "winreg", "winsound", "wsgiref", "xml",
        "xmlrpc", "zipapp", "zipfile", "zipimport", "zlib", "zoneinfo",
    ];
    STDLIB.contains(&root)
}

/// Port of `kl4a.codekb.architecture._collect_assignments`.
///
/// Grounded via MCP: Python only scans `tree.body` (top-level statements),
/// not the whole `ast.walk` — mirrored here by iterating `module_ast.body`
/// directly rather than [`all_stmts`].
fn collect_assignments(
    module_ast: &pyast_ast::ModModule,
    module: &str,
    path: &str,
    routers: &mut Vec<Value>,
    apps: &mut Vec<Value>,
    frameworks: &mut BTreeSet<String>,
) {
    for stmt in &module_ast.body {
        let pyast_ast::Stmt::Assign(assign) = stmt else { continue };
        let pyast_ast::Expr::Call(call) = assign.value.as_ref() else { continue };
        let name = match call.func.as_ref() {
            pyast_ast::Expr::Name(n) => Some(n.id.as_str()),
            pyast_ast::Expr::Attribute(a) => Some(a.attr.as_str()),
            _ => None,
        };
        let targets: Vec<&str> = assign
            .targets
            .iter()
            .filter_map(|t| match t {
                pyast_ast::Expr::Name(n) => Some(n.id.as_str()),
                _ => None,
            })
            .collect();
        let Some(var) = targets.first().copied() else { continue };
        match name {
            Some("APIRouter") => {
                frameworks.insert("fastapi".to_string());
                routers.push(json!({
                    "var": var,
                    "module": module,
                    "file": path,
                    "prefix": kwarg_str(&call.keywords, "prefix").unwrap_or_default(),
                    "tags": kwarg_list(&call.keywords, "tags"),
                }));
            }
            Some("FastAPI") => {
                frameworks.insert("fastapi".to_string());
                apps.push(json!({"var": var, "module": module, "file": path}));
            }
            _ => {}
        }
    }
}

/// Port of `kl4a.codekb.architecture._collect_endpoints`.
///
/// Grounded via MCP: Python only scans `tree.body` function/async-function
/// defs (not nested ones) and their direct `decorator_list`; a decorator
/// that isn't `Attribute(Name).Call` (`_decorator_target` returning a
/// `None` `call`) is skipped (`if router_var is None or call is None:
/// continue`).
fn collect_endpoints(
    module_ast: &pyast_ast::ModModule,
    li: &LineIndex,
    module: &str,
    path: &str,
    raw_endpoints: &mut Vec<Value>,
) {
    for stmt in &module_ast.body {
        let (handler, decorator_list, lineno) = match stmt {
            pyast_ast::Stmt::FunctionDef(f) => (f.name.as_str(), &f.decorator_list, start_line(li, f)),
            pyast_ast::Stmt::AsyncFunctionDef(f) => (f.name.as_str(), &f.decorator_list, start_line(li, f)),
            _ => continue,
        };
        for dec in decorator_list {
            let Some((router_var, attr, Some(call))) = decorator_target(dec) else { continue };
            let methods: Vec<String> = if HTTP_METHODS.contains(attr.as_str()) {
                vec![attr.to_uppercase()]
            } else if attr == "api_route" || attr == "add_api_route" {
                kwarg_list(&call.keywords, "methods").into_iter().map(|m| m.to_uppercase()).collect()
            } else {
                continue;
            };
            let route_path = call.args.first().and_then(|a| match a {
                pyast_ast::Expr::Constant(c) => match &c.value {
                    pyast_ast::Constant::Str(s) => Some(s.clone()),
                    _ => None,
                },
                _ => None,
            });
            let effective_methods = if methods.is_empty() { vec!["ANY".to_string()] } else { methods };
            for method in effective_methods {
                raw_endpoints.push(json!({
                    "method": method,
                    "route_path": route_path,
                    "router_var": router_var,
                    "handler": handler,
                    "handler_qname": format!("{module}.{handler}"),
                    "module": module,
                    "file": path,
                    "lineno": lineno,
                    "status_code": kwarg_str(&call.keywords, "status_code"),
                    "tags": kwarg_list(&call.keywords, "tags"),
                    "summary": kwarg_str(&call.keywords, "summary"),
                }));
            }
        }
    }
}

/// Port of `kl4a.codekb.architecture._collect_includes`.
///
/// Grounded via MCP: Python walks the whole AST (`ast.walk`), so an
/// `include_router` call nested anywhere (inside a function body, a
/// conditional, ...) is still found — mirrored here via [`all_exprs`].
fn collect_includes(module_ast: &pyast_ast::ModModule, module: &str, path: &str, includes: &mut Vec<Value>) {
    for expr in all_exprs(&module_ast.body) {
        let pyast_ast::Expr::Call(call) = expr else { continue };
        let pyast_ast::Expr::Attribute(func_attr) = call.func.as_ref() else { continue };
        if func_attr.attr.as_str() != "include_router" {
            continue;
        }
        let app_var = match func_attr.value.as_ref() {
            pyast_ast::Expr::Name(n) => Some(n.id.to_string()),
            _ => None,
        };
        let (router_var, token) = match call.args.first() {
            Some(pyast_ast::Expr::Name(n)) => (Some(n.id.to_string()), Some(n.id.to_string())),
            Some(pyast_ast::Expr::Attribute(a)) => {
                let token = match a.value.as_ref() {
                    pyast_ast::Expr::Name(n) => Some(n.id.to_string()),
                    _ => None,
                };
                (Some(a.attr.to_string()), token)
            }
            _ => (None, None),
        };
        includes.push(json!({
            "app_var": app_var,
            "router_var": router_var,
            "token": token,
            "module": module,
            "file": path,
            "prefix": kwarg_str(&call.keywords, "prefix").unwrap_or_default(),
        }));
    }
}

/// Port of `kl4a.codekb.architecture._collect_classes`.
///
/// Grounded via MCP: Python only scans `tree.body` top-level `ast.ClassDef`
/// nodes and their direct body statements (`ast.Assign`/`ast.AnnAssign`),
/// one indent level in — mirrored here by iterating `module_ast.body`
/// directly and only `cls.body` (not a deeper walk). `__tablename__`
/// stores the raw constant value (any type, not just `str`, per
/// `stmt.value.value`), so `tablename` is carried as a generic JSON `Value`
/// (round-tripping whatever type it is) rather than `Option<String>`.
fn collect_classes(
    module_ast: &pyast_ast::ModModule,
    li: &LineIndex,
    module: &str,
    path: &str,
    classes: &mut Vec<Value>,
) {
    for stmt in &module_ast.body {
        let pyast_ast::Stmt::ClassDef(cls) = stmt else { continue };
        let bases: Vec<String> = cls.bases.iter().map(unparse_expr).collect();
        let base_names: BTreeSet<String> = bases.iter().map(|b| b.rsplit('.').next().unwrap_or(b).to_string()).collect();
        let decorator_names: BTreeSet<String> = cls.decorator_list.iter().filter_map(decorator_bare_name).collect();

        let mut tablename: Value = Value::Null;
        let mut columns: Vec<Value> = Vec::new();
        let mut ann_fields: Vec<Value> = Vec::new();

        for body_stmt in &cls.body {
            match body_stmt {
                pyast_ast::Stmt::Assign(assign) => {
                    for t in &assign.targets {
                        let pyast_ast::Expr::Name(name_node) = t else { continue };
                        if name_node.id.as_str() == "__tablename__" {
                            if let pyast_ast::Expr::Constant(c) = assign.value.as_ref() {
                                tablename = match &c.value {
                                    pyast_ast::Constant::Str(s) => json!(s),
                                    pyast_ast::Constant::Bool(b) => json!(b),
                                    pyast_ast::Constant::Int(i) => json!(i.to_string()),
                                    pyast_ast::Constant::None => Value::Null,
                                    other => json!(stringify_constant(other)),
                                };
                            }
                        } else if let pyast_ast::Expr::Call(call) = assign.value.as_ref() {
                            let cname = match call.func.as_ref() {
                                pyast_ast::Expr::Name(n) => n.id.to_string(),
                                pyast_ast::Expr::Attribute(a) => a.attr.to_string(),
                                _ => String::new(),
                            };
                            if matches!(cname.as_str(), "Column" | "mapped_column" | "relationship") {
                                columns.push(json!({
                                    "name": name_node.id.as_str(),
                                    "kind": cname,
                                    "type": first_arg_type(call),
                                }));
                            }
                        }
                    }
                }
                pyast_ast::Stmt::AnnAssign(ann) => {
                    if let pyast_ast::Expr::Name(name_node) = ann.target.as_ref() {
                        ann_fields.push(json!({
                            "name": name_node.id.as_str(),
                            "type": unparse_expr(&ann.annotation),
                        }));
                    }
                }
                _ => {}
            }
        }

        classes.push(json!({
            "name": cls.name.as_str(),
            "qname": format!("{module}.{}", cls.name.as_str()),
            "module": module,
            "file": path,
            "lineno": start_line(li, cls),
            "bases": bases,
            "base_names": base_names,
            "decorator_names": decorator_names,
            "tablename": tablename,
            "columns": columns,
            "ann_fields": ann_fields,
        }));
    }
}

// ---------------------------------------------------------------------
// Post-processing (faithful 1:1 ports — no AST involved)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.architecture._classify_classes`, including its
/// nested `transitive` closure (ported as the local `transitive` fn below,
/// same fixed-point-over-`base_names` mechanism, not unrolled).
fn classify_classes(classes: &[Value]) -> (Vec<Value>, Vec<Value>) {
    fn get_str<'a>(c: &'a Value, k: &str) -> &'a str {
        c.get(k).and_then(Value::as_str).unwrap_or_default()
    }
    let base_names_of = |c: &Value| -> BTreeSet<String> {
        c.get("base_names")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    };

    let transitive = |seed: &dyn Fn(&Value) -> bool| -> BTreeSet<String> {
        let mut members: BTreeSet<String> = classes
            .iter()
            .filter(|c| seed(c))
            .map(|c| get_str(c, "name").to_string())
            .collect();
        loop {
            let mut changed = false;
            for c in classes {
                let name = get_str(c, "name").to_string();
                if members.contains(&name) {
                    continue;
                }
                if base_names_of(c).intersection(&members).next().is_some() {
                    members.insert(name);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        members
    };

    let orm_names = transitive(&|c: &Value| {
        base_names_of(c).contains("Base") || c.get("tablename").map(|t| !t.is_null()).unwrap_or(false)
    });
    let schema_names = transitive(&|c: &Value| base_names_of(c).contains("BaseModel"));

    let mut orm_models = Vec::new();
    let mut schemas = Vec::new();
    for c in classes {
        let name = get_str(c, "name").to_string();
        let qname = get_str(c, "qname").to_string();
        let bnames = base_names_of(c);
        let tablename = c.get("tablename").cloned().unwrap_or(Value::Null);
        let columns = c.get("columns").cloned().unwrap_or_else(|| json!([]));
        let has_columns = columns.as_array().map(|a| !a.is_empty()).unwrap_or(false);

        let is_orm = orm_names.contains(&name)
            && (!tablename.is_null() || bnames.contains("Base") || has_columns);
        let is_schema = schema_names.contains(&name);
        let decorator_names: BTreeSet<String> = c
            .get("decorator_names")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        let is_plain_data = !is_orm && !is_schema && is_plain_data_class(&decorator_names, &bnames);

        if is_orm && !(is_schema && tablename.is_null() && !has_columns) {
            orm_models.push(json!({
                "id": bounded_id("model", &qname, 112),
                "qualified_name": qname,
                "name": name,
                "module": c.get("module"),
                "file": c.get("file"),
                "line": c.get("lineno"),
                "table": tablename,
                "bases": c.get("bases"),
                "columns": columns,
                "symbol_id": maybe_symbol(&qname),
            }));
        } else if is_schema || is_plain_data {
            schemas.push(json!({
                "id": bounded_id("schema", &qname, 112),
                "qualified_name": qname,
                "name": name,
                "module": c.get("module"),
                "file": c.get("file"),
                "line": c.get("lineno"),
                "bases": c.get("bases"),
                "fields": c.get("ann_fields"),
                "symbol_id": maybe_symbol(&qname),
                "kind": if is_schema { "pydantic" } else { "plain-data-class" },
            }));
        }
    }
    (orm_models, schemas)
}

/// Port of `kl4a.codekb.architecture._module_mount_prefix`.
fn module_mount_prefix(module: &str, router_var: &str, includes: &[Value]) -> String {
    let last = module.rsplit('.').next().unwrap_or(module);
    for inc in includes {
        if inc.get("token").and_then(Value::as_str) == Some(last) {
            return inc.get("prefix").and_then(Value::as_str).unwrap_or_default().to_string();
        }
    }
    for inc in includes {
        if inc.get("router_var").and_then(Value::as_str) == Some(router_var) {
            return inc.get("prefix").and_then(Value::as_str).unwrap_or_default().to_string();
        }
    }
    String::new()
}

/// Port of `kl4a.codekb.architecture._resolve_endpoints`.
fn resolve_endpoints(
    raw: &[Value],
    routers: &[Value],
    includes: &[Value],
    symbol_ids: &HashSet<String>,
) -> Vec<Value> {
    let mut router_by_key: HashMap<(String, String), &Value> = HashMap::new();
    for r in routers {
        let module = r.get("module").and_then(Value::as_str).unwrap_or_default().to_string();
        let var = r.get("var").and_then(Value::as_str).unwrap_or_default().to_string();
        router_by_key.insert((module, var), r);
    }

    let mut endpoints = Vec::new();
    let mut seen: HashMap<String, u32> = HashMap::new();
    for e in raw {
        let module = e.get("module").and_then(Value::as_str).unwrap_or_default();
        let router_var = e.get("router_var").and_then(Value::as_str).unwrap_or_default();
        let router = router_by_key.get(&(module.to_string(), router_var.to_string()));
        let router_prefix = router
            .and_then(|r| r.get("prefix"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mount_prefix = module_mount_prefix(module, router_var, includes);
        let route_path_val = e.get("route_path").cloned().unwrap_or(Value::Null);
        let resolved = !route_path_val.is_null();
        let route = route_path_val.as_str().unwrap_or_default();
        let mut full = join_path(&[&mount_prefix, router_prefix, route]);
        if resolved && full.is_empty() {
            full = "/".to_string();
        }
        let handler_qname = e.get("handler_qname").and_then(Value::as_str).unwrap_or_default();
        let handler_symbol_id = code_symbol_id_for(handler_qname);
        let handler_present = symbol_ids.contains(&handler_symbol_id);
        let method = e.get("method").and_then(Value::as_str).unwrap_or_default();
        let handler = e.get("handler").and_then(Value::as_str).unwrap_or_default();
        let base = bounded_id(
            "endpoint",
            &format!("{method}-{}-{handler}", if !full.is_empty() { &full } else { route }),
            120,
        );
        let n = *seen.get(&base).unwrap_or(&0);
        seen.insert(base.clone(), n + 1);
        let eid = if n == 0 { base.clone() } else { format!("{base}-{n}") };

        endpoints.push(json!({
            "id": eid,
            "kind": "http",
            "framework": "fastapi",
            "method": method,
            "path": if resolved { full.clone() } else { "(dynamic)".to_string() },
            "route_path": route,
            "router_prefix": router_prefix,
            "mount_prefix": mount_prefix,
            "router_var": router_var,
            "handler": handler,
            "handler_qname": handler_qname,
            "handler_symbol_id": if handler_present { Value::String(handler_symbol_id) } else { Value::Null },
            "module": module,
            "file": e.get("file"),
            "line": e.get("lineno"),
            "status_code": e.get("status_code"),
            "tags": e.get("tags"),
            "summary": e.get("summary"),
            "path_resolved": resolved,
        }));
    }
    endpoints.sort_by(|a, b| {
        let ap = a.get("path").and_then(Value::as_str).unwrap_or_default();
        let bp = b.get("path").and_then(Value::as_str).unwrap_or_default();
        let am = a.get("method").and_then(Value::as_str).unwrap_or_default();
        let bm = b.get("method").and_then(Value::as_str).unwrap_or_default();
        (ap, am).cmp(&(bp, bm))
    });
    endpoints
}

/// Port of `kl4a.codekb.architecture._router_view`.
fn router_view(routers: &[Value], includes: &[Value], endpoints: &[Value]) -> Vec<Value> {
    let mut counts: HashMap<(String, String), u32> = HashMap::new();
    for e in endpoints {
        let module = e.get("module").and_then(Value::as_str).unwrap_or_default().to_string();
        let router_var = e.get("router_var").and_then(Value::as_str).unwrap_or_default().to_string();
        *counts.entry((module, router_var)).or_insert(0) += 1;
    }
    let mounted_tokens: HashSet<&str> = includes
        .iter()
        .filter_map(|inc| inc.get("token").and_then(Value::as_str))
        .collect();
    let mounted_router_vars: HashSet<&str> = includes
        .iter()
        .filter_map(|inc| inc.get("router_var").and_then(Value::as_str))
        .collect();

    let mut view = Vec::new();
    for r in routers {
        let module = r.get("module").and_then(Value::as_str).unwrap_or_default();
        let var = r.get("var").and_then(Value::as_str).unwrap_or_default();
        let last = module.rsplit('.').next().unwrap_or(module);
        let mount_prefix = module_mount_prefix(module, var, includes);
        let mounted = mounted_tokens.contains(last) || mounted_router_vars.contains(var);
        let endpoint_count = *counts.get(&(module.to_string(), var.to_string())).unwrap_or(&0);
        view.push(json!({
            "var": var,
            "module": module,
            "file": r.get("file"),
            "prefix": r.get("prefix"),
            "mount_prefix": mount_prefix,
            "mounted": mounted,
            "endpoint_count": endpoint_count,
            "tags": r.get("tags").cloned().unwrap_or_else(|| json!([])),
        }));
    }
    view.sort_by(|a, b| {
        let ac = a.get("endpoint_count").and_then(Value::as_u64).unwrap_or(0);
        let bc = b.get("endpoint_count").and_then(Value::as_u64).unwrap_or(0);
        let av = a.get("var").and_then(Value::as_str).unwrap_or_default();
        let bv = b.get("var").and_then(Value::as_str).unwrap_or_default();
        bc.cmp(&ac).then(av.cmp(bv))
    });
    view
}

// ---------------------------------------------------------------------
// OKF Markdown writers (faithful 1:1 ports — no AST involved)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.architecture._write_endpoint_docs`.
fn write_endpoint_docs(bundle_dir: &Path, endpoints: &[Value]) -> Result<()> {
    for e in endpoints {
        let kind = e.get("kind").and_then(Value::as_str).unwrap_or("http");
        let fm = json!({
            "type": "Code Endpoint",
            "title": endpoint_title(e),
            "status": "stable",
            "codekb": {
                "endpoint": {
                    "id": e.get("id"), "kind": e.get("kind"), "framework": e.get("framework"),
                    "method": e.get("method"), "path": e.get("path"), "route_path": e.get("route_path"),
                    "router_prefix": e.get("router_prefix"), "mount_prefix": e.get("mount_prefix"),
                    "router_var": e.get("router_var"), "handler_qname": e.get("handler_qname"),
                    "handler_symbol_id": e.get("handler_symbol_id"), "module": e.get("module"),
                    "file": e.get("file"), "line": e.get("line"), "status_code": e.get("status_code"),
                    "tags": e.get("tags"), "name": e.get("name"), "path_resolved": e.get("path_resolved"),
                }
            },
        });
        let handler_qname = e.get("handler_qname").and_then(Value::as_str).unwrap_or("");
        let handler_link = match e.get("handler_symbol_id").and_then(Value::as_str) {
            Some(sid) => format!("- Handler: [{handler_qname}](../symbols/{sid}.md)"),
            None => format!("- Handler: {}", if handler_qname.is_empty() { "\u{2014}" } else { handler_qname }),
        };
        let mut lines = vec![
            format!("# {}", endpoint_title(e)),
            String::new(),
            format!("- Framework: `{}`", e.get("framework").and_then(Value::as_str).unwrap_or("fastapi")),
        ];
        if kind == "http" {
            let router_var = e.get("router_var").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("\u{2014}");
            let router_prefix = e.get("router_prefix").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("/");
            let mount_prefix = e.get("mount_prefix").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("/");
            lines.push(format!("- Router: `{router_var}` (prefix `{router_prefix}`, mount `{mount_prefix}`)"));
        } else {
            let name = e.get("name").and_then(Value::as_str).filter(|s| !s.is_empty())
                .unwrap_or_else(|| e.get("path").and_then(Value::as_str).unwrap_or(""));
            lines.push(format!("- Name: `{name}`"));
        }
        lines.push(handler_link);
        lines.push(format!("- Source: `{}:{}`",
            e.get("file").and_then(Value::as_str).unwrap_or(""),
            e.get("line").cloned().unwrap_or(Value::Null),
        ));
        if let Some(tags) = e.get("tags").and_then(Value::as_array) {
            if !tags.is_empty() {
                let joined: Vec<&str> = tags.iter().filter_map(Value::as_str).collect();
                lines.push(format!("- Tags: {}", joined.join(", ")));
            }
        }
        if let Some(status_code) = e.get("status_code") {
            if !status_code.is_null() {
                lines.push(format!("- Status code: {status_code}"));
            }
        }
        let id = e.get("id").and_then(Value::as_str).unwrap_or("unknown");
        crate::okf_writer::write_markdown(
            &bundle_dir.join("code").join("endpoints").join(format!("{id}.md")),
            &fm,
            &format!("{}\n", lines.join("\n")),
        )?;
    }
    Ok(())
}

/// Port of `kl4a.codekb.architecture._write_model_docs`.
fn write_model_docs(bundle_dir: &Path, models: &[Value]) -> Result<()> {
    for m in models {
        let fm = json!({
            "type": "Code Data Model",
            "title": m.get("qualified_name"),
            "status": "stable",
            "codekb": {"data_model": {
                "id": m.get("id"), "qualified_name": m.get("qualified_name"), "name": m.get("name"),
                "module": m.get("module"), "file": m.get("file"), "line": m.get("line"),
                "table": m.get("table"), "bases": m.get("bases"), "columns": m.get("columns"),
                "symbol_id": m.get("symbol_id"),
            }},
        });
        let cols = m.get("columns").and_then(Value::as_array).map(|cols| {
            if cols.is_empty() {
                "- (no columns detected)".to_string()
            } else {
                cols.iter()
                    .map(|c| format!(
                        "- `{}` \u{2014} {}({})",
                        c.get("name").and_then(Value::as_str).unwrap_or(""),
                        c.get("kind").and_then(Value::as_str).unwrap_or(""),
                        c.get("type").and_then(Value::as_str).unwrap_or(""),
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }).unwrap_or_else(|| "- (no columns detected)".to_string());
        let bases_joined = m.get("bases").and_then(Value::as_array)
            .map(|b| b.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "\u{2014}".to_string());
        let table = m.get("table").and_then(Value::as_str).unwrap_or("\u{2014}");
        let body = format!(
            "# {}\n\n- Table: `{}`\n- Bases: {}\n- Source: `{}:{}`\n\n## Columns\n\n{}\n",
            m.get("qualified_name").and_then(Value::as_str).unwrap_or(""),
            table, bases_joined,
            m.get("file").and_then(Value::as_str).unwrap_or(""),
            m.get("line").cloned().unwrap_or(Value::Null),
            cols,
        );
        let id = m.get("id").and_then(Value::as_str).unwrap_or("unknown");
        crate::okf_writer::write_markdown(
            &bundle_dir.join("code").join("data-models").join(format!("{id}.md")),
            &fm,
            &body,
        )?;
    }
    Ok(())
}

/// Port of `kl4a.codekb.architecture._write_schema_docs`.
fn write_schema_docs(bundle_dir: &Path, schemas: &[Value]) -> Result<()> {
    for s in schemas {
        let fm = json!({
            "type": "Code Schema",
            "title": s.get("qualified_name"),
            "status": "stable",
            "codekb": {"schema": {
                "id": s.get("id"), "qualified_name": s.get("qualified_name"), "name": s.get("name"),
                "module": s.get("module"), "file": s.get("file"), "line": s.get("line"),
                "bases": s.get("bases"), "fields": s.get("fields"), "symbol_id": s.get("symbol_id"),
                "kind": s.get("kind"),
            }},
        });
        let fields = s.get("fields").and_then(Value::as_array).map(|fields| {
            if fields.is_empty() {
                "- (no annotated fields)".to_string()
            } else {
                fields.iter()
                    .map(|f| format!(
                        "- `{}`: {}",
                        f.get("name").and_then(Value::as_str).unwrap_or(""),
                        f.get("type").and_then(Value::as_str).unwrap_or(""),
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }).unwrap_or_else(|| "- (no annotated fields)".to_string());
        let bases_joined = s.get("bases").and_then(Value::as_array)
            .map(|b| b.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "\u{2014}".to_string());
        let kind = s.get("kind").and_then(Value::as_str).unwrap_or("pydantic");
        let body = format!(
            "# {}\n\n- Kind: `{}`\n- Bases: {}\n- Source: `{}:{}`\n\n## Fields\n\n{}\n",
            s.get("qualified_name").and_then(Value::as_str).unwrap_or(""),
            kind, bases_joined,
            s.get("file").and_then(Value::as_str).unwrap_or(""),
            s.get("line").cloned().unwrap_or(Value::Null),
            fields,
        );
        let id = s.get("id").and_then(Value::as_str).unwrap_or("unknown");
        crate::okf_writer::write_markdown(
            &bundle_dir.join("code").join("schemas").join(format!("{id}.md")),
            &fm,
            &body,
        )?;
    }
    Ok(())
}

/// Port of `kl4a.codekb.architecture._write_dependency_docs`.
fn write_dependency_docs(bundle_dir: &Path, deps: &[Value]) -> Result<()> {
    for d in deps {
        let modules = d.get("modules").and_then(Value::as_array).cloned().unwrap_or_default();
        let fm = json!({
            "type": "Code Dependency",
            "title": d.get("package"),
            "status": "stable",
            "codekb": {"dependency": {
                "id": d.get("id"), "package": d.get("package"), "category": d.get("category"),
                "import_count": modules.len(), "modules": modules,
            }},
        });
        let mods = modules.iter().take(60)
            .filter_map(Value::as_str)
            .map(|m| format!("- `{m}`"))
            .collect::<Vec<_>>()
            .join("\n");
        let body = format!(
            "# {}\n\n- Category: {}\n- Imported by {} module(s)\n\n## Importers\n\n{}\n",
            d.get("package").and_then(Value::as_str).unwrap_or(""),
            d.get("category").and_then(Value::as_str).unwrap_or(""),
            modules.len(),
            mods,
        );
        let id = d.get("id").and_then(Value::as_str).unwrap_or("unknown");
        crate::okf_writer::write_markdown(
            &bundle_dir.join("code").join("dependencies").join(format!("{id}.md")),
            &fm,
            &body,
        )?;
    }
    Ok(())
}

/// Port of `kl4a.codekb.architecture._merge_adapter_architecture`.
///
/// Folds each non-Python adapter's views into the shared state. Adapters
/// own their own keys and their own summary counts; nothing here knows what
/// a copybook is.
fn merge_adapter_architecture(
    bundle_dir: &Path,
    inventory: &Value,
    symbols: &[Value],
    mut state: Value,
) -> Value {
    let symbol_state = load_json(&bundle_dir.join(crate::state::STATE_DIR).join("code_symbols.json"));
    let relations = load_json(&bundle_dir.join(crate::state::STATE_DIR).join("code_relations.json"))
        .get("relations")
        .cloned()
        .unwrap_or_else(|| json!([]));

    let contributed = crate::adapters::cobol_architecture::detect_cobol_architecture(
        inventory.get("sources").cloned().unwrap_or_else(|| json!([])),
        symbol_state.get("modules").cloned().unwrap_or_else(|| json!([])),
        Value::Array(symbols.to_vec()),
        relations,
    );

    if let Value::Object(contributed_map) = &contributed {
        for (key, value) in contributed_map {
            if key == "summary" {
                if let (Some(summary), Value::Object(new_summary)) =
                    (state.get_mut("summary"), value)
                {
                    if let Value::Object(summary_map) = summary {
                        for (k, v) in new_summary {
                            summary_map.insert(k.clone(), v.clone());
                        }
                    }
                }
            } else {
                state[key] = value.clone();
            }
        }
        if !contributed_map.is_empty() {
            let mut frameworks: BTreeSet<String> = state
                .get("frameworks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            frameworks.insert("cobol".to_string());
            state["frameworks"] = json!(frameworks.into_iter().collect::<Vec<_>>());
        }
    }
    state
}

/// Port of `kl4a.codekb.architecture.detect_architecture`.
///
/// Detects architecture for whatever languages the bundle holds. Python
/// contributes the FastAPI/SQLAlchemy/Pydantic views; COBOL contributes its
/// own. Each adapter adds its keys to one shared state file, and the
/// workbench builds the Architecture section from whichever keys came back
/// populated — so a repository is never offered a view of another stack.
pub fn detect_architecture(bundle_dir: &Path) -> Result<Value> {
    let inventory = load_json(&bundle_dir.join(crate::state::STATE_DIR).join("code_inventory.json"));
    let symbols = load_json(&bundle_dir.join(crate::state::STATE_DIR).join("code_symbols.json"))
        .get("symbols")
        .cloned()
        .unwrap_or_else(|| json!([]))
        .as_array()
        .cloned()
        .unwrap_or_default();
    let symbol_ids: HashSet<String> = symbols
        .iter()
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(str::to_string))
        .collect();
    let sources: Vec<Value> = inventory
        .get("sources")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.get("language").and_then(Value::as_str) == Some("python"))
        .collect();

    let mut repo_roots: HashSet<String> = REPO_ROOTS_DEFAULT.iter().map(|s| s.to_string()).collect();
    for src in &sources {
        let path = src.get("path").and_then(Value::as_str).unwrap_or_default();
        let module = crate::parse::module_name_for(path);
        let parts: Vec<&str> = module.split('.').collect();
        if let Some(first) = parts.first() {
            if !first.is_empty() {
                repo_roots.insert(first.to_string());
            }
        }
        if parts.len() > 1 && !parts[1].is_empty() {
            // A module's own qualified_name is derived from its path relative
            // to the *scanned* root, which for a monorepo scan (e.g. scanning
            // `tools/` rather than `tools/kl4a/`) commonly carries one extra
            // segment (e.g. "kl4a.codekb.foo") that real Python import
            // statements never include (`import codekb...` resolves against
            // the actual installed package root, not the scan root). Without
            // this, a repo's own sibling packages get misclassified as
            // external dependencies.
            repo_roots.insert(parts[1].to_string());
        }
    }

    let mut routers: Vec<Value> = Vec::new();
    let mut apps: Vec<Value> = Vec::new();
    let mut raw_endpoints: Vec<Value> = Vec::new();
    let mut includes: Vec<Value> = Vec::new();
    let mut classes: Vec<Value> = Vec::new();
    let mut dep_hits: BTreeMap<String, Value> = BTreeMap::new();
    let mut frameworks: BTreeSet<String> = BTreeSet::new();

    for src in &sources {
        let path = src.get("path").and_then(Value::as_str).unwrap_or_default();
        let module = crate::parse::module_name_for(path);
        let original_path = src.get("original_path").and_then(Value::as_str).unwrap_or_default();
        let original = bundle_dir.join(original_path);
        if !original.exists() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&original) else {
            continue;
        };
        // Grounded via MCP (`detect_architecture`'s own per-file loop):
        // `try: tree = ast.parse(...) except SyntaxError: continue` — a file
        // that fails to parse contributes nothing, exactly like every other
        // `ast.parse` call site in this codebase (`parse.py`,
        // `entrypoints.py`).
        let Ok(parsed) = rustpython_parser::parse(&text, Mode::Module, path) else {
            continue;
        };
        let pyast_ast::Mod::Module(module_ast) = parsed else {
            continue;
        };
        let li = LineIndex::new(&text);

        collect_imports(&module_ast, &module, path, &repo_roots, &mut dep_hits, &mut frameworks);
        collect_assignments(&module_ast, &module, path, &mut routers, &mut apps, &mut frameworks);
        collect_endpoints(&module_ast, &li, &module, path, &mut raw_endpoints);
        collect_includes(&module_ast, &module, path, &mut includes);
        collect_classes(&module_ast, &li, &module, path, &mut classes);
    }

    let (orm_models, schemas) = classify_classes(&classes);
    let mut endpoints = resolve_endpoints(&raw_endpoints, &routers, &includes, &symbol_ids);

    // Non-FastAPI entrypoints (Flask, Django, Click/argparse, Celery) in the
    // same schema — delegated wholesale; see module-level cross-batch note.
    let (extra_endpoints, extra_frameworks, flask_vars) =
        crate::entrypoints::collect_entrypoints(bundle_dir, &sources, &symbol_ids);
    if !flask_vars.is_empty() {
        // A Flask `@app.get(...)` is also grabbed by the FastAPI shortcut
        // collector; drop those so the Flask detector's richer record
        // (mount prefix) wins.
        endpoints.retain(|e| {
            e.get("router_var")
                .and_then(Value::as_str)
                .map(|v| !flask_vars.contains(v))
                .unwrap_or(true)
        });
    }
    endpoints.extend(extra_endpoints);
    frameworks.extend(extra_frameworks);

    let mut dependencies: Vec<Value> = dep_hits.into_values().collect();
    dependencies.sort_by(|a, b| {
        let am = a.get("modules").and_then(Value::as_array).map(|v| v.len()).unwrap_or(0);
        let bm = b.get("modules").and_then(Value::as_array).map(|v| v.len()).unwrap_or(0);
        let ap = a.get("package").and_then(Value::as_str).unwrap_or_default();
        let bp = b.get("package").and_then(Value::as_str).unwrap_or_default();
        bm.cmp(&am).then(ap.cmp(bp))
    });

    // ---- materialize OKF docs ----
    for rel in ARCH_DIRS {
        reset_dir(&bundle_dir.join(rel))?;
    }
    write_endpoint_docs(bundle_dir, &endpoints)?;
    write_model_docs(bundle_dir, &orm_models)?;
    write_schema_docs(bundle_dir, &schemas)?;
    write_dependency_docs(bundle_dir, &dependencies)?;

    let http_endpoints = endpoints.iter()
        .filter(|e| e.get("kind").and_then(Value::as_str).unwrap_or("http") == "http")
        .count();
    let cli_commands = endpoints.iter()
        .filter(|e| e.get("kind").and_then(Value::as_str) == Some("cli"))
        .count();
    let tasks = endpoints.iter()
        .filter(|e| e.get("kind").and_then(Value::as_str) == Some("task"))
        .count();

    let mut state = json!({
        "frameworks": frameworks.into_iter().collect::<Vec<_>>(),
        "summary": {
            "endpoints": endpoints.len(),
            "http_endpoints": http_endpoints,
            "cli_commands": cli_commands,
            "tasks": tasks,
            "routers": routers.len(),
            "apps": apps.len(),
            "data_models": orm_models.len(),
            "schemas": schemas.len(),
            "dependencies": dependencies.len(),
        },
        "endpoints": endpoints.clone(),
        "routers": router_view(&routers, &includes, &endpoints),
        "apps": apps,
        "data_models": orm_models,
        "schemas": schemas,
        "dependencies": dependencies,
    });
    state = merge_adapter_architecture(bundle_dir, &inventory, &symbols, state);
    crate::state::write_code_state(bundle_dir, "code_architecture.json", &state)?;
    Ok(state)
}
