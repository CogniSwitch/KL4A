//! Port of `kl4a/codekb/entrypoints.py` — non-FastAPI Python entrypoint
//! detection (Flask, Django, Click, argparse, Celery, plain scripts).
//!
//! Every symbol is grounded via tools-code MCP `code_symbols_get` against
//! `kl4a.codekb.entrypoints.*`; Python excerpts are quoted per function.
//!
//! # Real Python-source parsing via `rustpython_parser`
//!
//! The whole file operates on Python's `ast` module tree shape.
//! [`iter_python_asts`] (Python: `_iter_python_asts`, whose only job is
//! `ast.parse(source_text)`) now really parses every Python source with
//! `rustpython_parser` instead of returning an empty stub. Rather than
//! rewrite the six detector functions against `rustpython_parser`'s own
//! (much larger) AST shape, this file **keeps its local [`pyast`] mirror**
//! (a minimal, purpose-built shape covering only the node kinds these
//! detectors actually inspect: `FunctionDef`/`AsyncFunctionDef`, `Assign`,
//! `If`/`Compare`, `Call`/`Attribute`/`Name`/`Constant`/`List`/`Tuple`) and
//! adds a translator ([`translate_module`]) from the real parsed tree into
//! this mirror — the six detectors themselves (`scan_flask_objects`,
//! `collect_flask`, `collect_django`, `collect_click`, `collect_argparse`,
//! `collect_celery`, `collect_plain_script`) and `finalize` are **completely
//! unchanged**, since keeping the mirror is far less churn than rewriting
//! six already-carefully-grounded detectors against a different node shape.
//! The one behavioral improvement this translator makes over the mirror's
//! previous (pre-real-parser) design: [`pyast::Stmt`] gained a `Compound`
//! variant so `for`/`while`/`with`/`try`/`match` bodies are still walked
//! into by [`pyast::walk_stmts`]/[`pyast::walk_all_exprs`] — matching real
//! `ast.walk`'s exhaustive traversal (a genuine gap in the mirror's
//! original guesswork design, which only ever modeled `FunctionDef`/`If`
//! nesting) — without touching any of the six detectors, none of which
//! pattern-match on `Compound` themselves.
//!
//! # Cross-batch / cross-module vendoring (see also `ids.rs`'s own note)
//!
//! - `kl4a.codekb.architecture._kwarg_str` / `_kwarg_list` / `_join_path`
//!   (architecture.py, not in this batch) are vendored below as
//!   `kwarg_str`/`kwarg_list`/`join_path`, grounded via their own
//!   `code_symbols_get` evidence, following the same vendor-and-reconcile
//!   convention `ids.rs` already established for `kl4a/kl4a/ids.py`.
//! - `kl4a.codekb.parse.module_name_for` (parse.py, not in this batch) is
//!   vendored below as `module_name_for`, same reasoning.
//! - `crate::ids::{bounded_id, code_symbol_id_for}` — already ported in
//!   this crate (`ids.rs`), reused as-is (no vendoring needed).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde_json::{json, Value};

use crate::ids::{bounded_id, code_symbol_id_for};

/// Minimal local mirror of the subset of Python's `ast` module that this
/// file's pattern matching needs. **Not** a general Python parser — see the
/// module-level doc comment above for what this is and is not.
pub mod pyast {
    #[derive(Debug, Clone)]
    pub enum PyConst {
        Str(String),
        Other,
    }

    #[derive(Debug, Clone)]
    pub enum Expr {
        Name(String),
        Attribute {
            value: Box<Expr>,
            attr: String,
        },
        Call {
            func: Box<Expr>,
            args: Vec<Expr>,
            keywords: Vec<Keyword>,
            lineno: usize,
        },
        Constant(PyConst),
        List(Vec<Expr>),
        Tuple(Vec<Expr>),
        /// A `Compare` with exactly one operator and one comparator (`a == b`).
        /// Python's `ast.Compare` supports chained comparisons
        /// (`a == b == c`) with multiple `ops`/`comparators`; this file only
        /// ever needs to recognize `__name__ == "__main__"`, so the model is
        /// narrowed to the single-comparator case (`_is_main_guard` rejects
        /// anything else the same way the Python does via `len(test.ops) != 1`).
        Compare {
            left: Box<Expr>,
            eq: bool,
            comparator: Box<Expr>,
        },
        Other,
    }

    #[derive(Debug, Clone)]
    pub struct Keyword {
        pub arg: Option<String>,
        pub value: Expr,
    }

    #[derive(Debug, Clone)]
    pub enum Stmt {
        FunctionDef {
            name: String,
            #[allow(dead_code)]
            is_async: bool,
            decorator_list: Vec<Expr>,
            body: Vec<Stmt>,
            lineno: usize,
        },
        Assign {
            targets: Vec<Expr>,
            value: Expr,
            #[allow(dead_code)]
            lineno: usize,
        },
        If {
            test: Expr,
            body: Vec<Stmt>,
            orelse: Vec<Stmt>,
            lineno: usize,
        },
        ExprStmt {
            value: Expr,
            #[allow(dead_code)]
            lineno: usize,
        },
        /// Any other compound statement (`for`/`while`/`with`/`try`/`match`)
        /// that this file's six detectors never pattern-match on directly,
        /// but whose nested bodies must still be reachable by
        /// [`walk_stmts`] — mirroring real `ast.walk`'s exhaustive
        /// traversal (see the module-level doc comment). `bodies` holds
        /// every nested statement list this node owns (a `for`'s body +
        /// orelse, a `try`'s body + each handler's body + orelse +
        /// finally, a `match`'s per-case bodies, ...); `test_or_iter` holds
        /// the one expression worth surfacing to [`walk_all_exprs`] (a
        /// `while`/`if`-style test, a `for`'s iterable, a `match`'s
        /// subject) when there is one.
        Compound {
            bodies: Vec<Vec<Stmt>>,
            test_or_iter: Option<Expr>,
            lineno: usize,
        },
        Other {
            lineno: usize,
        },
    }

    impl Stmt {
        pub fn lineno(&self) -> usize {
            match self {
                Stmt::FunctionDef { lineno, .. }
                | Stmt::Assign { lineno, .. }
                | Stmt::If { lineno, .. }
                | Stmt::ExprStmt { lineno, .. }
                | Stmt::Compound { lineno, .. }
                | Stmt::Other { lineno } => *lineno,
            }
        }
    }

    #[derive(Debug, Clone, Default)]
    pub struct Module {
        pub body: Vec<Stmt>,
    }

    /// Every statement reachable from `stmts`, including nested bodies
    /// (inside `FunctionDef`/`If` blocks) — the `Stmt`-typed slice of what
    /// `ast.walk(tree)` visits.
    pub fn walk_stmts(stmts: &[Stmt]) -> Vec<&Stmt> {
        let mut out = Vec::new();
        for s in stmts {
            out.push(s);
            match s {
                Stmt::FunctionDef { body, .. } => out.extend(walk_stmts(body)),
                Stmt::If { body, orelse, .. } => {
                    out.extend(walk_stmts(body));
                    out.extend(walk_stmts(orelse));
                }
                Stmt::Compound { bodies, .. } => {
                    for body in bodies {
                        out.extend(walk_stmts(body));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// The direct (not recursing into nested statement bodies) `Expr`
    /// fields of a single statement.
    fn direct_exprs(stmt: &Stmt) -> Vec<&Expr> {
        match stmt {
            Stmt::FunctionDef { decorator_list, .. } => decorator_list.iter().collect(),
            Stmt::Assign { targets, value, .. } => {
                let mut v: Vec<&Expr> = targets.iter().collect();
                v.push(value);
                v
            }
            Stmt::If { test, .. } => vec![test],
            Stmt::ExprStmt { value, .. } => vec![value],
            Stmt::Compound { test_or_iter, .. } => test_or_iter.iter().collect(),
            Stmt::Other { .. } => vec![],
        }
    }

    /// Recursively descends into every sub-expression of `expr` (call
    /// args/keywords, attribute values, list/tuple elements, compare
    /// operands) — the `Expr`-typed slice of what `ast.walk(tree)` visits.
    pub fn walk_expr_tree<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
        out.push(expr);
        match expr {
            Expr::Attribute { value, .. } => walk_expr_tree(value, out),
            Expr::Call { func, args, keywords, .. } => {
                walk_expr_tree(func, out);
                for a in args {
                    walk_expr_tree(a, out);
                }
                for k in keywords {
                    walk_expr_tree(&k.value, out);
                }
            }
            Expr::List(items) | Expr::Tuple(items) => {
                for i in items {
                    walk_expr_tree(i, out);
                }
            }
            Expr::Compare { left, comparator, .. } => {
                walk_expr_tree(left, out);
                walk_expr_tree(comparator, out);
            }
            Expr::Name(_) | Expr::Constant(_) | Expr::Other => {}
        }
    }

    /// Every `Expr` node reachable anywhere in `module` (combines
    /// [`walk_stmts`] with [`walk_expr_tree`]) — together with
    /// [`walk_stmts`], the `Expr`+`Stmt` equivalent of Python's
    /// `ast.walk(tree)` for the node kinds this file matches on.
    pub fn walk_all_exprs(module: &Module) -> Vec<&Expr> {
        let mut out = Vec::new();
        for stmt in walk_stmts(&module.body) {
            for e in direct_exprs(stmt) {
                walk_expr_tree(e, &mut out);
            }
        }
        out
    }

    /// A raw-pointer identity key for an `Expr`, standing in for Python's
    /// `id(node)` — used only to correlate the *same* AST node across two
    /// independent passes over the same tree (see `_collect_argparse`'s
    /// `target_by_call`), never for hashing/ordering content.
    pub fn expr_identity(expr: &Expr) -> usize {
        expr as *const Expr as usize
    }
}

use pyast::{Expr, Keyword, Module, PyConst, Stmt};
use rustpython_parser::ast as rpy;
use rustpython_parser::Mode;

// ---------------------------------------------------------------------
// Byte-offset -> 1-based line number, and the `rustpython_parser` ->
// local-`pyast`-mirror translator that replaces `iter_python_asts`'s old
// empty stub. Vendored independently in `parse.rs`/`architecture.rs` too —
// each file's own small copy, per this codebase's established "vendor and
// reconcile" convention (see this file's own `_kwarg_str`/`_kwarg_list`/
// `_join_path`/`module_name_for` vendoring above).
// ---------------------------------------------------------------------

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
fn start_line<T: rustpython_parser::ast::Ranged>(li: &LineIndex, node: &T) -> usize {
    li.at(u32::from(node.range().start()) as usize)
}

/// Translates one parsed `rustpython_parser` module into this file's local
/// [`pyast::Module`] mirror.
fn translate_module(m: &rpy::ModModule, li: &LineIndex) -> Module {
    Module {
        body: m.body.iter().filter_map(|s| translate_stmt(s, li)).collect(),
    }
}

fn translate_body(stmts: &[rpy::Stmt], li: &LineIndex) -> Vec<Stmt> {
    stmts.iter().filter_map(|s| translate_stmt(s, li)).collect()
}

fn translate_stmt(s: &rpy::Stmt, li: &LineIndex) -> Option<Stmt> {
    match s {
        rpy::Stmt::FunctionDef(f) => Some(Stmt::FunctionDef {
            name: f.name.to_string(),
            is_async: false,
            decorator_list: f.decorator_list.iter().map(|d| translate_expr(d, li)).collect(),
            body: translate_body(&f.body, li),
            lineno: start_line(li, f),
        }),
        rpy::Stmt::AsyncFunctionDef(f) => Some(Stmt::FunctionDef {
            name: f.name.to_string(),
            is_async: true,
            decorator_list: f.decorator_list.iter().map(|d| translate_expr(d, li)).collect(),
            body: translate_body(&f.body, li),
            lineno: start_line(li, f),
        }),
        rpy::Stmt::Assign(a) => Some(Stmt::Assign {
            targets: a.targets.iter().map(|t| translate_expr(t, li)).collect(),
            value: translate_expr(&a.value, li),
            lineno: start_line(li, a),
        }),
        rpy::Stmt::If(i) => Some(Stmt::If {
            test: translate_expr(&i.test, li),
            body: translate_body(&i.body, li),
            orelse: translate_body(&i.orelse, li),
            lineno: start_line(li, i),
        }),
        rpy::Stmt::Expr(e) => Some(Stmt::ExprStmt {
            value: translate_expr(&e.value, li),
            lineno: start_line(li, e),
        }),
        rpy::Stmt::For(f) => Some(Stmt::Compound {
            bodies: vec![translate_body(&f.body, li), translate_body(&f.orelse, li)],
            test_or_iter: Some(translate_expr(&f.iter, li)),
            lineno: start_line(li, f),
        }),
        rpy::Stmt::AsyncFor(f) => Some(Stmt::Compound {
            bodies: vec![translate_body(&f.body, li), translate_body(&f.orelse, li)],
            test_or_iter: Some(translate_expr(&f.iter, li)),
            lineno: start_line(li, f),
        }),
        rpy::Stmt::While(w) => Some(Stmt::Compound {
            bodies: vec![translate_body(&w.body, li), translate_body(&w.orelse, li)],
            test_or_iter: Some(translate_expr(&w.test, li)),
            lineno: start_line(li, w),
        }),
        rpy::Stmt::With(w) => Some(Stmt::Compound {
            bodies: vec![translate_body(&w.body, li)],
            test_or_iter: None,
            lineno: start_line(li, w),
        }),
        rpy::Stmt::AsyncWith(w) => Some(Stmt::Compound {
            bodies: vec![translate_body(&w.body, li)],
            test_or_iter: None,
            lineno: start_line(li, w),
        }),
        rpy::Stmt::Try(t) => {
            let mut bodies = vec![translate_body(&t.body, li)];
            for h in &t.handlers {
                let rpy::ExceptHandler::ExceptHandler(eh) = h;
                bodies.push(translate_body(&eh.body, li));
            }
            bodies.push(translate_body(&t.orelse, li));
            bodies.push(translate_body(&t.finalbody, li));
            Some(Stmt::Compound { bodies, test_or_iter: None, lineno: start_line(li, t) })
        }
        rpy::Stmt::Match(m) => {
            let bodies = m.cases.iter().map(|case| translate_body(&case.body, li)).collect();
            Some(Stmt::Compound {
                bodies,
                test_or_iter: Some(translate_expr(&m.subject, li)),
                lineno: start_line(li, m),
            })
        }
        // Everything else (`Return`/`Raise`/`Import`/`ClassDef`/`Pass`/...)
        // isn't matched on directly by any of this file's six detectors;
        // kept as `Other` purely so its line number is available and so
        // `walk_stmts` doesn't need to know about it.
        other => Some(Stmt::Other { lineno: start_line(li, other) }),
    }
}

fn translate_expr(e: &rpy::Expr, li: &LineIndex) -> Expr {
    match e {
        rpy::Expr::Name(n) => Expr::Name(n.id.to_string()),
        rpy::Expr::Attribute(a) => Expr::Attribute {
            value: Box::new(translate_expr(&a.value, li)),
            attr: a.attr.to_string(),
        },
        rpy::Expr::Call(c) => Expr::Call {
            func: Box::new(translate_expr(&c.func, li)),
            args: c.args.iter().map(|a| translate_expr(a, li)).collect(),
            keywords: c
                .keywords
                .iter()
                .map(|k| Keyword { arg: k.arg.as_ref().map(|s| s.to_string()), value: translate_expr(&k.value, li) })
                .collect(),
            lineno: start_line(li, c),
        },
        rpy::Expr::Constant(c) => match &c.value {
            rpy::Constant::Str(s) => Expr::Constant(PyConst::Str(s.clone())),
            _ => Expr::Constant(PyConst::Other),
        },
        rpy::Expr::List(l) => Expr::List(l.elts.iter().map(|e| translate_expr(e, li)).collect()),
        rpy::Expr::Tuple(t) => Expr::Tuple(t.elts.iter().map(|e| translate_expr(e, li)).collect()),
        rpy::Expr::Compare(c) if c.ops.len() == 1 && matches!(c.ops[0], rpy::CmpOp::Eq) => Expr::Compare {
            left: Box::new(translate_expr(&c.left, li)),
            eq: true,
            comparator: Box::new(translate_expr(&c.comparators[0], li)),
        },
        _ => Expr::Other,
    }
}

/// Port of `kl4a.codekb.entrypoints.DJANGO_URL_FUNCS` (`{'path', 're_path', 'url'}`).
pub const DJANGO_URL_FUNCS: [&str; 3] = ["path", "re_path", "url"];
/// Port of `kl4a.codekb.entrypoints.HTTP_METHOD_ATTRS`
/// (`{'get', 'post', 'put', 'delete', 'patch', 'options', 'head'}`).
pub const HTTP_METHOD_ATTRS: [&str; 7] = ["get", "post", "put", "delete", "patch", "options", "head"];

// ---------------------------------------------------------------------
// Small AST-accessor helpers (`_callee_name`, `_const_str`, `_first_str_arg`,
// `_kwarg_value`, `_single_target`, `_view_ref`, `_is_main_guard`) — all
// defined directly in `entrypoints.py` itself.
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.entrypoints._callee_name`.
///
/// ```python
/// def _callee_name(func: ast.AST) -> str | None:
///     if isinstance(func, ast.Name): return func.id
///     if isinstance(func, ast.Attribute): return func.attr
///     return None
/// ```
fn callee_name(func: &Expr) -> Option<&str> {
    match func {
        Expr::Name(id) => Some(id.as_str()),
        Expr::Attribute { attr, .. } => Some(attr.as_str()),
        _ => None,
    }
}

/// Port of `kl4a.codekb.entrypoints._const_str`.
///
/// ```python
/// def _const_str(node: ast.AST | None) -> str | None:
///     if isinstance(node, ast.Constant) and isinstance(node.value, str): return node.value
///     return None
/// ```
fn const_str<'a>(node: Option<&'a Expr>) -> Option<&'a str> {
    match node {
        Some(Expr::Constant(PyConst::Str(s))) => Some(s.as_str()),
        _ => None,
    }
}

/// Port of `kl4a.codekb.entrypoints._first_str_arg`.
///
/// ```python
/// def _first_str_arg(call: ast.Call | None) -> str | None:
///     if call and call.args: return _const_str(call.args[0])
///     return None
/// ```
fn first_str_arg(call: &Expr) -> Option<&str> {
    match call {
        Expr::Call { args, .. } if !args.is_empty() => const_str(Some(&args[0])),
        _ => None,
    }
}

/// Port of `kl4a.codekb.entrypoints._kwarg_value`.
///
/// ```python
/// def _kwarg_value(call: ast.Call | None, name: str) -> ast.AST | None:
///     if not call: return None
///     for kw in call.keywords:
///         if kw.arg == name: return kw.value
///     return None
/// ```
fn kwarg_value<'a>(call: Option<&'a Expr>, name: &str) -> Option<&'a Expr> {
    match call {
        Some(Expr::Call { keywords, .. }) => {
            keywords.iter().find(|k| k.arg.as_deref() == Some(name)).map(|k| &k.value)
        }
        _ => None,
    }
}

/// Port of `kl4a.codekb.entrypoints._single_target`.
///
/// ```python
/// def _single_target(targets: list[ast.AST]) -> str | None:
///     names = [t.id for t in targets if isinstance(t, ast.Name)]
///     return names[0] if names else None
/// ```
fn single_target(targets: &[Expr]) -> Option<&str> {
    targets.iter().find_map(|t| match t {
        Expr::Name(id) => Some(id.as_str()),
        _ => None,
    })
}

/// Reconstructs the full dotted-path source text for a `Name`/`Attribute`
/// chain, i.e. what real `ast.unparse(node)` produces for an `ast.Attribute`
/// whose base eventually bottoms out at a bare `ast.Name` (`views.profile`,
/// `views.ArticleView.as_view`, ...). Walks all the way to the root instead
/// of stopping at the immediate `.attr` segment. Returns `None` when the
/// base isn't a `Name`/`Attribute` chain (e.g. a subscript or call as the
/// base) — this minimal AST has no general unparser for those shapes, so
/// `view_ref`'s `Attribute` branch falls back to `node.attr` there, mirroring
/// Python's `try: ast.unparse(node) except Exception: return node.attr`.
fn unparse_attribute_chain(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(id) => Some(id.clone()),
        Expr::Attribute { value, attr } => {
            unparse_attribute_chain(value).map(|base| format!("{base}.{attr}"))
        }
        _ => None,
    }
}

/// Port of `kl4a.codekb.entrypoints._view_ref`.
///
/// ```python
/// def _view_ref(node: ast.AST | None) -> str | None:
///     """Best-effort textual reference for a view/handler passed by value."""
///     if node is None: return None
///     if isinstance(node, ast.Name): return node.id
///     if isinstance(node, ast.Attribute):
///         try: return ast.unparse(node)
///         except Exception: return node.attr
///     if isinstance(node, ast.Call): return _view_ref(node.func)
///     if isinstance(node, ast.Constant) and isinstance(node.value, str): return node.value
///     return None
/// ```
///
/// The `ast.Attribute` branch's `try: ast.unparse(node)` reproduces the full
/// dotted-path source text (e.g. `views.profile`, `views.ArticleView.as_view`)
/// by walking the whole chain back to its root `Name`, not just the last
/// `.attr` segment — reconstructed here via [`unparse_attribute_chain`]. The
/// `Call` branch (`MyView.as_view()`) recurses into `node.func`, which is
/// itself this same `Attribute` chain, so `views.ArticleView.as_view` comes
/// out whole. Only when the chain's base isn't a plain `Name`/`Attribute`
/// (this minimal AST's stand-in for `ast.unparse` raising) does this fall
/// back to `attr` alone, matching the `except Exception: return node.attr`
/// branch.
fn view_ref(node: Option<&Expr>) -> Option<String> {
    match node {
        None => None,
        Some(Expr::Name(id)) => Some(id.clone()),
        Some(attr_node @ Expr::Attribute { attr, .. }) => {
            Some(unparse_attribute_chain(attr_node).unwrap_or_else(|| attr.clone()))
        }
        Some(Expr::Call { func, .. }) => view_ref(Some(func)),
        Some(Expr::Constant(PyConst::Str(s))) => Some(s.clone()),
        _ => None,
    }
}

/// Port of `kl4a.codekb.entrypoints._is_main_guard`.
///
/// ```python
/// def _is_main_guard(node: ast.AST) -> bool:
///     """Match `if __name__ == "__main__":` in either operand order."""
///     if not isinstance(node, ast.If): return False
///     test = node.test
///     if not isinstance(test, ast.Compare) or len(test.ops) != 1: return False
///     if not isinstance(test.ops[0], ast.Eq): return False
///     sides = (test.left, test.comparators[0])
///     names = {s.id for s in sides if isinstance(s, ast.Name)}
///     constants = {s.value for s in sides if isinstance(s, ast.Constant)}
///     return "__name__" in names and "__main__" in constants
/// ```
fn is_main_guard(stmt: &Stmt) -> bool {
    let test = match stmt {
        Stmt::If { test, .. } => test,
        _ => return false,
    };
    let (left, eq, comparator) = match test {
        Expr::Compare { left, eq, comparator } => (left.as_ref(), *eq, comparator.as_ref()),
        _ => return false,
    };
    if !eq {
        return false;
    }
    let sides = [left, comparator];
    let has_name = sides.iter().any(|s| matches!(s, Expr::Name(n) if n == "__name__"));
    let has_main_const = sides
        .iter()
        .any(|s| matches!(s, Expr::Constant(PyConst::Str(v)) if v == "__main__"));
    has_name && has_main_const
}

// ---------------------------------------------------------------------
// Vendored cross-module helpers (grounded via `code_symbols_get`, not part
// of this batch — see module doc).
// ---------------------------------------------------------------------

/// Vendored from `kl4a.codekb.architecture._kwarg_str` (architecture.py, not
/// in this batch).
///
/// ```python
/// def _kwarg_str(call: ast.Call, name: str) -> str | None:
///     for kw in call.keywords:
///         if kw.arg == name and isinstance(kw.value, ast.Constant): return str(kw.value.value)
///     return None
/// ```
///
/// Python stringifies *any* constant value (`str(kw.value.value)`); this
/// minimal AST's `Constant` only distinguishes `Str` from `Other`, so only
/// string-valued keyword constants are recognized here — UNCONFIRMED for a
/// non-string constant keyword argument (e.g. `url_prefix=None`), which
/// Python would stringify to `"None"` and this returns `None` for instead.
fn kwarg_str<'a>(call: &'a Expr, name: &str) -> Option<&'a str> {
    match call {
        Expr::Call { keywords, .. } => keywords.iter().find_map(|k| {
            if k.arg.as_deref() == Some(name) {
                if let Expr::Constant(PyConst::Str(s)) = &k.value {
                    return Some(s.as_str());
                }
            }
            None
        }),
        _ => None,
    }
}

/// Vendored from `kl4a.codekb.architecture._kwarg_list` (architecture.py,
/// not in this batch).
///
/// ```python
/// def _kwarg_list(call: ast.Call, name: str) -> list[str]:
///     for kw in call.keywords:
///         if kw.arg == name and isinstance(kw.value, (ast.List, ast.Tuple)):
///             return [str(e.value) for e in kw.value.elts if isinstance(e, ast.Constant)]
///     return []
/// ```
fn kwarg_list(call: &Expr, name: &str) -> Vec<String> {
    match call {
        Expr::Call { keywords, .. } => keywords
            .iter()
            .find(|k| k.arg.as_deref() == Some(name))
            .map(|k| match &k.value {
                Expr::List(items) | Expr::Tuple(items) => items
                    .iter()
                    .filter_map(|e| match e {
                        Expr::Constant(PyConst::Str(s)) => Some(s.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Vendored from `kl4a.codekb.architecture._join_path` (architecture.py,
/// not in this batch).
///
/// ```python
/// def _join_path(*parts: str) -> str:
///     segs = []
///     for p in parts:
///         if not p: continue
///         segs.append("/" + p.strip("/"))
///     joined = "".join(segs)
///     return joined or ""
/// ```
fn join_path(parts: &[&str]) -> String {
    let mut joined = String::new();
    for p in parts {
        if p.is_empty() {
            continue;
        }
        joined.push('/');
        joined.push_str(p.trim_matches('/'));
    }
    joined
}

/// Vendored from `kl4a.codekb.parse.module_name_for` (parse.py, not in this
/// batch).
///
/// ```python
/// def module_name_for(relative_path: str) -> str:
///     path = Path(relative_path)
///     parts = list(path.with_suffix("").parts)
///     if parts and parts[-1] == "__init__": parts = parts[:-1]
///     return ".".join(parts) or path.stem
/// ```
fn module_name_for(relative_path: &str) -> String {
    let path = Path::new(relative_path);
    let stem_path = path.with_extension("");
    let mut parts: Vec<String> = stem_path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    if parts.last().map(|s| s == "__init__").unwrap_or(false) {
        parts.pop();
    }
    if parts.is_empty() {
        stem_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
    } else {
        parts.join(".")
    }
}

// ---------------------------------------------------------------------
// Raw-entry constructors (`_http_raw`, `_named_raw`).
// ---------------------------------------------------------------------

fn handler_qname_value(module: &str, handler: Option<&str>) -> Value {
    match handler {
        None => Value::Null,
        Some(h) if h.is_empty() => Value::String(String::new()),
        Some(h) if !h.contains('.') => Value::String(format!("{module}.{h}")),
        Some(h) => Value::String(h.to_string()),
    }
}

/// Port of `kl4a.codekb.entrypoints._http_raw`.
///
/// ```python
/// def _http_raw(framework, method, route, mount_prefix, var, handler, module, path, lineno, name=None) -> dict:
///     resolved = route is not None
///     handler_qname = f"{module}.{handler}" if handler and "." not in handler else handler
///     return {"kind": "http", "framework": framework, "method": method, "route_path": route or "",
///             "mount_prefix": mount_prefix or "", "router_prefix": "", "router_var": var or "",
///             "handler": handler or "", "handler_qname": handler_qname, "module": module,
///             "file": path, "line": lineno, "name": name, "path_resolved": resolved}
/// ```
#[allow(clippy::too_many_arguments)]
fn http_raw(
    framework: &str,
    method: &str,
    route: Option<&str>,
    mount_prefix: &str,
    var: &str,
    handler: Option<&str>,
    module: &str,
    path: &str,
    lineno: usize,
    name: Option<&str>,
) -> Value {
    json!({
        "kind": "http",
        "framework": framework,
        "method": method,
        "route_path": route.unwrap_or(""),
        "mount_prefix": mount_prefix,
        "router_prefix": "",
        "router_var": var,
        "handler": handler.unwrap_or(""),
        "handler_qname": handler_qname_value(module, handler),
        "module": module,
        "file": path,
        "line": lineno,
        "name": name,
        "path_resolved": route.is_some(),
    })
}

/// Port of `kl4a.codekb.entrypoints._named_raw`.
///
/// ```python
/// def _named_raw(framework, method, name, handler, module, path, lineno) -> dict:
///     """CLI commands and Celery tasks: the command/task name is the 'path'."""
///     handler_qname = f"{module}.{handler}" if handler and "." not in handler else handler
///     return {"kind": "cli" if method == "CLI" else "task", "framework": framework, "method": method,
///             "route_path": name or "", "mount_prefix": "", "router_prefix": "", "router_var": "",
///             "handler": handler or "", "handler_qname": handler_qname, "module": module,
///             "file": path, "line": lineno, "name": name, "path_resolved": name is not None}
/// ```
fn named_raw(
    framework: &str,
    method: &str,
    name: Option<&str>,
    handler: Option<&str>,
    module: &str,
    path: &str,
    lineno: usize,
) -> Value {
    json!({
        "kind": if method == "CLI" { "cli" } else { "task" },
        "framework": framework,
        "method": method,
        "route_path": name.unwrap_or(""),
        "mount_prefix": "",
        "router_prefix": "",
        "router_var": "",
        "handler": handler.unwrap_or(""),
        "handler_qname": handler_qname_value(module, handler),
        "module": module,
        "file": path,
        "line": lineno,
        "name": name,
        "path_resolved": name.is_some(),
    })
}

// ---------------------------------------------------------------------
// Framework detectors.
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
struct FlaskApp {
    #[allow(dead_code)]
    module: String,
}

#[derive(Debug, Clone)]
struct FlaskBlueprint {
    #[allow(dead_code)]
    module: String,
    prefix: String,
}

/// Port of `kl4a.codekb.entrypoints._scan_flask_objects`.
///
/// ```python
/// def _scan_flask_objects(tree, module, flask_apps, flask_blueprints, register_prefixes):
///     for node in ast.walk(tree):
///         if isinstance(node, ast.Assign) and isinstance(node.value, ast.Call):
///             name = _callee_name(node.value.func)
///             var = _single_target(node.targets)
///             if not var: continue
///             if name == "Flask": flask_apps[var] = {"module": module}
///             elif name == "Blueprint":
///                 flask_blueprints[var] = {"module": module, "prefix": _kwarg_str(node.value, "url_prefix") or ""}
///         elif (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
///               and node.func.attr == "register_blueprint" and node.args):
///             bp = node.args[0]
///             bp_var = bp.id if isinstance(bp, ast.Name) else (bp.attr if isinstance(bp, ast.Attribute) else None)
///             override = _kwarg_str(node, "url_prefix")
///             if bp_var and override is not None: register_prefixes[bp_var] = override
/// ```
///
/// Ported as two targeted passes (`Assign` nodes via [`pyast::walk_stmts`],
/// then `Call` nodes via [`pyast::walk_all_exprs`]) instead of one unified
/// `isinstance`-branching loop, since this crate's minimal AST keeps
/// statements and expressions as separate types. The two Python branches
/// are mutually exclusive node kinds that only ever write into disjoint
/// maps, so splitting the single Python loop into two Rust loops over the
/// same tree is observably identical.
fn scan_flask_objects(
    module_ast: &Module,
    module: &str,
    flask_apps: &mut HashMap<String, FlaskApp>,
    flask_blueprints: &mut HashMap<String, FlaskBlueprint>,
    register_prefixes: &mut HashMap<String, String>,
) {
    for stmt in pyast::walk_stmts(&module_ast.body) {
        if let Stmt::Assign { targets, value, .. } = stmt {
            if let Expr::Call { func, .. } = value {
                if let (Some(name), Some(var)) = (callee_name(func), single_target(targets)) {
                    if name == "Flask" {
                        flask_apps.insert(var.to_string(), FlaskApp { module: module.to_string() });
                    } else if name == "Blueprint" {
                        let prefix = kwarg_str(value, "url_prefix").unwrap_or("").to_string();
                        flask_blueprints.insert(
                            var.to_string(),
                            FlaskBlueprint { module: module.to_string(), prefix },
                        );
                    }
                }
            }
        }
    }

    for expr in pyast::walk_all_exprs(module_ast) {
        let Expr::Call { func, args, .. } = expr else { continue };
        let Expr::Attribute { attr, .. } = func.as_ref() else { continue };
        if attr != "register_blueprint" || args.is_empty() {
            continue;
        }
        let bp_var = match &args[0] {
            Expr::Name(id) => Some(id.clone()),
            Expr::Attribute { attr, .. } => Some(attr.clone()),
            _ => None,
        };
        let override_prefix = kwarg_str(expr, "url_prefix");
        if let (Some(bp_var), Some(over)) = (bp_var, override_prefix) {
            register_prefixes.insert(bp_var, over.to_string());
        }
    }
}

/// Port of `kl4a.codekb.entrypoints._collect_flask`, including its nested
/// `mount_prefix(var)` closure.
///
/// ```python
/// def _collect_flask(tree, module, path, flask_apps, flask_blueprints, register_prefixes, raw, frameworks):
///     known = set(flask_apps) | set(flask_blueprints)
///     def mount_prefix(var: str) -> str:
///         if var in register_prefixes: return register_prefixes[var]
///         bp = flask_blueprints.get(var)
///         return bp["prefix"] if bp else ""
///     for node in tree.body:
///         if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)): continue
///         for dec in node.decorator_list:
///             call = dec if isinstance(dec, ast.Call) else None
///             attr_node = call.func if call else dec
///             if not (isinstance(attr_node, ast.Attribute) and isinstance(attr_node.value, ast.Name)): continue
///             var, attr = attr_node.value.id, attr_node.attr
///             if var not in known: continue
///             if attr == "route":
///                 methods = [m.upper() for m in (_kwarg_list(call, "methods") or ["GET"])] if call else ["GET"]
///             elif attr in HTTP_METHOD_ATTRS:
///                 methods = [attr.upper()]
///             else:
///                 continue
///             route = _first_str_arg(call) if call else None
///             mp = mount_prefix(var)
///             for method in methods:
///                 raw.append(_http_raw("flask", method, route, mp, var, node.name, module, path, node.lineno))
///     for node in ast.walk(tree):
///         if not (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
///                 and node.func.attr == "add_url_rule" and isinstance(node.func.value, ast.Name)):
///             continue
///         var = node.func.value.id
///         if var not in known: continue
///         route = _first_str_arg(node)
///         methods = [m.upper() for m in (_kwarg_list(node, "methods") or ["GET"])]
///         handler = _view_ref(_kwarg_value(node, "view_func"))
///         for method in methods:
///             raw.append(_http_raw("flask", method, route, mount_prefix(var), var, handler, module, path, node.lineno))
///     if raw and any(r["framework"] == "flask" for r in raw): frameworks.add("flask")
/// ```
#[allow(clippy::too_many_arguments)]
fn collect_flask(
    module_ast: &Module,
    module: &str,
    path: &str,
    flask_apps: &HashMap<String, FlaskApp>,
    flask_blueprints: &HashMap<String, FlaskBlueprint>,
    register_prefixes: &HashMap<String, String>,
    raw: &mut Vec<Value>,
    frameworks: &mut HashSet<String>,
) {
    let known: HashSet<String> = flask_apps.keys().cloned().chain(flask_blueprints.keys().cloned()).collect();
    let mount_prefix = |var: &str| -> String {
        if let Some(p) = register_prefixes.get(var) {
            return p.clone();
        }
        flask_blueprints.get(var).map(|bp| bp.prefix.clone()).unwrap_or_default()
    };

    // decorator form: @app.route(...) / @bp.route(...) / @app.get(...)
    for node in &module_ast.body {
        let Stmt::FunctionDef { name: fn_name, decorator_list, lineno, .. } = node else { continue };
        for dec in decorator_list {
            let call = if matches!(dec, Expr::Call { .. }) { Some(dec) } else { None };
            let attr_node = match call {
                Some(Expr::Call { func, .. }) => func.as_ref(),
                _ => dec,
            };
            let (var, attr) = match attr_node {
                Expr::Attribute { value, attr } => match value.as_ref() {
                    Expr::Name(id) => (id.as_str(), attr.as_str()),
                    _ => continue,
                },
                _ => continue,
            };
            if !known.contains(var) {
                continue;
            }
            let methods: Vec<String> = if attr == "route" {
                match call {
                    Some(c) => {
                        let m = kwarg_list(c, "methods");
                        if m.is_empty() {
                            vec!["GET".to_string()]
                        } else {
                            m.into_iter().map(|s| s.to_uppercase()).collect()
                        }
                    }
                    None => vec!["GET".to_string()],
                }
            } else if HTTP_METHOD_ATTRS.contains(&attr) {
                vec![attr.to_uppercase()]
            } else {
                continue;
            };
            let route = call.and_then(first_str_arg);
            let mp = mount_prefix(var);
            for method in &methods {
                raw.push(http_raw("flask", method, route, &mp, var, Some(fn_name.as_str()), module, path, *lineno, None));
            }
        }
    }

    // imperative form: app.add_url_rule("/p", view_func=fn, methods=[...])
    for expr in pyast::walk_all_exprs(module_ast) {
        let Expr::Call { func, args: _, lineno, .. } = expr else { continue };
        let Expr::Attribute { value, attr } = func.as_ref() else { continue };
        if attr != "add_url_rule" {
            continue;
        }
        let Expr::Name(var) = value.as_ref() else { continue };
        if !known.contains(var.as_str()) {
            continue;
        }
        let route = first_str_arg(expr);
        let m = kwarg_list(expr, "methods");
        let methods: Vec<String> = if m.is_empty() {
            vec!["GET".to_string()]
        } else {
            m.into_iter().map(|s| s.to_uppercase()).collect()
        };
        let handler = view_ref(kwarg_value(Some(expr), "view_func"));
        for method in &methods {
            raw.push(http_raw(
                "flask",
                method,
                route,
                &mount_prefix(var),
                var,
                handler.as_deref(),
                module,
                path,
                *lineno,
                None,
            ));
        }
    }

    if raw.iter().any(|r| r.get("framework").and_then(Value::as_str) == Some("flask")) {
        frameworks.insert("flask".to_string());
    }
}

/// Port of `kl4a.codekb.entrypoints._collect_django`.
///
/// ```python
/// def _collect_django(tree, module, path, raw, frameworks):
///     """Collect path()/re_path()/url() entries. Django resolves the leading
///     route prefix through nested include() across files; that cross-file
///     mount is not reconstructed here, so `path` is the local route and
///     `mount_prefix` is ""."""
///     found = False
///     for node in ast.walk(tree):
///         if not (isinstance(node, ast.Call) and node.args): continue
///         fname = _callee_name(node.func)
///         if fname not in DJANGO_URL_FUNCS: continue
///         route = _const_str(node.args[0])
///         if route is None: continue  # skip include(...) and non-literal routes
///         view = node.args[1] if len(node.args) > 1 else _kwarg_value(node, "view")
///         handler = _view_ref(view)
///         name = _kwarg_str(node, "name")
///         raw.append(_http_raw("django", "ANY", route, "", "", handler, module, path, node.lineno, name=name))
///         found = True
///     if found: frameworks.add("django")
/// ```
fn collect_django(module_ast: &Module, module: &str, path: &str, raw: &mut Vec<Value>, frameworks: &mut HashSet<String>) {
    let mut found = false;
    for expr in pyast::walk_all_exprs(module_ast) {
        let Expr::Call { func, args, lineno, .. } = expr else { continue };
        if args.is_empty() {
            continue;
        }
        let Some(fname) = callee_name(func) else { continue };
        if !DJANGO_URL_FUNCS.contains(&fname) {
            continue;
        }
        let Some(route) = const_str(Some(&args[0])) else { continue };
        let view = if args.len() > 1 { Some(&args[1]) } else { kwarg_value(Some(expr), "view") };
        let handler = view_ref(view);
        let name = kwarg_str(expr, "name");
        raw.push(http_raw("django", "ANY", Some(route), "", "", handler.as_deref(), module, path, *lineno, name));
        found = true;
    }
    if found {
        frameworks.insert("django".to_string());
    }
}

/// Port of `kl4a.codekb.entrypoints._collect_click`.
///
/// ```python
/// def _collect_click(tree, module, path, raw, frameworks):
///     for node in tree.body:
///         if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)): continue
///         for dec in node.decorator_list:
///             call = dec if isinstance(dec, ast.Call) else None
///             attr_node = call.func if call else dec
///             if not isinstance(attr_node, ast.Attribute): continue
///             if attr_node.attr not in ("command", "group"): continue
///             name = (_first_str_arg(call) if call else None) or (_kwarg_str(call, "name") if call else None) or node.name
///             raw.append(_named_raw("click", "CLI", name, node.name, module, path, node.lineno))
///             frameworks.add("click")
/// ```
fn collect_click(module_ast: &Module, module: &str, path: &str, raw: &mut Vec<Value>, frameworks: &mut HashSet<String>) {
    for node in &module_ast.body {
        let Stmt::FunctionDef { name: fn_name, decorator_list, lineno, .. } = node else { continue };
        for dec in decorator_list {
            let call = if matches!(dec, Expr::Call { .. }) { Some(dec) } else { None };
            let attr_node = match call {
                Some(Expr::Call { func, .. }) => func.as_ref(),
                _ => dec,
            };
            let Expr::Attribute { attr, .. } = attr_node else { continue };
            if attr != "command" && attr != "group" {
                continue;
            }
            let name = call
                .and_then(first_str_arg)
                .or_else(|| call.and_then(|c| kwarg_str(c, "name")))
                .unwrap_or(fn_name.as_str());
            raw.push(named_raw("click", "CLI", Some(name), Some(fn_name.as_str()), module, path, *lineno));
            frameworks.insert("click".to_string());
        }
    }
}

/// Port of `kl4a.codekb.entrypoints._collect_argparse`.
///
/// ```python
/// def _collect_argparse(tree, module, path, raw, frameworks):
///     handlers: dict[str, str] = {}
///     for node in ast.walk(tree):
///         if (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
///                 and node.func.attr == "set_defaults" and isinstance(node.func.value, ast.Name)):
///             func = _kwarg_value(node, "func")
///             if func is not None: handlers[node.func.value.id] = _view_ref(func)
///     target_by_call: dict[int, str] = {}
///     for node in ast.walk(tree):
///         if (isinstance(node, ast.Assign) and isinstance(node.value, ast.Call)
///                 and isinstance(node.value.func, ast.Attribute) and node.value.func.attr == "add_parser"):
///             var = _single_target(node.targets)
///             if var: target_by_call[id(node.value)] = var
///     for node in ast.walk(tree):
///         if not (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
///                 and node.func.attr == "add_parser" and node.args):
///             continue
///         name = _const_str(node.args[0])
///         if name is None: continue
///         handler = handlers.get(target_by_call.get(id(node)))
///         raw.append(_named_raw("argparse", "CLI", name, handler, module, path, node.lineno))
///         frameworks.add("argparse")
/// ```
///
/// `id(node)` correlates the *same* AST node object across the three passes
/// (a `Call` reached generically vs. reached as an `Assign`'s `.value`);
/// mirrored via [`pyast::expr_identity`] (a raw pointer into the same
/// owned tree, used purely as an opaque key).
fn collect_argparse(module_ast: &Module, module: &str, path: &str, raw: &mut Vec<Value>, frameworks: &mut HashSet<String>) {
    let mut handlers: HashMap<String, Option<String>> = HashMap::new();
    for expr in pyast::walk_all_exprs(module_ast) {
        let Expr::Call { func, .. } = expr else { continue };
        let Expr::Attribute { value, attr } = func.as_ref() else { continue };
        if attr != "set_defaults" {
            continue;
        }
        let Expr::Name(var) = value.as_ref() else { continue };
        if let Some(func_val) = kwarg_value(Some(expr), "func") {
            handlers.insert(var.clone(), view_ref(Some(func_val)));
        }
    }

    let mut target_by_call: HashMap<usize, String> = HashMap::new();
    for stmt in pyast::walk_stmts(&module_ast.body) {
        let Stmt::Assign { targets, value, .. } = stmt else { continue };
        let Expr::Call { func, .. } = value else { continue };
        let Expr::Attribute { attr, .. } = func.as_ref() else { continue };
        if attr != "add_parser" {
            continue;
        }
        if let Some(var) = single_target(targets) {
            target_by_call.insert(pyast::expr_identity(value), var.to_string());
        }
    }

    for expr in pyast::walk_all_exprs(module_ast) {
        let Expr::Call { func, args, lineno, .. } = expr else { continue };
        let Expr::Attribute { attr, .. } = func.as_ref() else { continue };
        if attr != "add_parser" || args.is_empty() {
            continue;
        }
        let Some(name) = const_str(Some(&args[0])) else { continue };
        let handler = target_by_call
            .get(&pyast::expr_identity(expr))
            .and_then(|var| handlers.get(var))
            .cloned()
            .flatten();
        raw.push(named_raw("argparse", "CLI", Some(name), handler.as_deref(), module, path, *lineno));
        frameworks.insert("argparse".to_string());
    }
}

/// Port of `kl4a.codekb.entrypoints._collect_celery`.
///
/// ```python
/// def _collect_celery(tree, module, path, raw, frameworks):
///     for node in tree.body:
///         if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)): continue
///         for dec in node.decorator_list:
///             call = dec if isinstance(dec, ast.Call) else None
///             target = call.func if call else dec
///             is_task = (isinstance(target, ast.Name) and target.id == "shared_task") or (
///                 isinstance(target, ast.Attribute) and target.attr in ("task", "shared_task"))
///             if not is_task: continue
///             name = (_kwarg_str(call, "name") if call else None) or f"{module}.{node.name}"
///             raw.append(_named_raw("celery", "TASK", name, node.name, module, path, node.lineno))
///             frameworks.add("celery")
/// ```
fn collect_celery(module_ast: &Module, module: &str, path: &str, raw: &mut Vec<Value>, frameworks: &mut HashSet<String>) {
    for node in &module_ast.body {
        let Stmt::FunctionDef { name: fn_name, decorator_list, lineno, .. } = node else { continue };
        for dec in decorator_list {
            let call = if matches!(dec, Expr::Call { .. }) { Some(dec) } else { None };
            let target = match call {
                Some(Expr::Call { func, .. }) => func.as_ref(),
                _ => dec,
            };
            let is_task = match target {
                Expr::Name(id) => id == "shared_task",
                Expr::Attribute { attr, .. } => attr == "task" || attr == "shared_task",
                _ => false,
            };
            if !is_task {
                continue;
            }
            let name = call
                .and_then(|c| kwarg_str(c, "name"))
                .map(str::to_string)
                .unwrap_or_else(|| format!("{module}.{fn_name}"));
            raw.push(named_raw("celery", "TASK", Some(&name), Some(fn_name.as_str()), module, path, *lineno));
            frameworks.insert("celery".to_string());
        }
    }
}

/// Port of `kl4a.codekb.entrypoints._collect_plain_script`.
///
/// ```python
/// def _collect_plain_script(tree, module, path, raw, frameworks):
///     main_def = next((n for n in tree.body if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)) and n.name == "main"), None)
///     guard = next((n for n in tree.body if _is_main_guard(n)), None)
///     if main_def is None and guard is None: return
///     lineno = main_def.lineno if main_def else guard.lineno
///     handler = main_def.name if main_def else None
///     raw.append(_named_raw("script", "CLI", module, handler, module, path, lineno))
///     frameworks.add("script")
/// ```
fn collect_plain_script(module_ast: &Module, module: &str, path: &str, raw: &mut Vec<Value>, frameworks: &mut HashSet<String>) {
    let main_def = module_ast.body.iter().find(|n| matches!(n, Stmt::FunctionDef { name, .. } if name == "main"));
    let guard = module_ast.body.iter().find(|n| is_main_guard(n));
    if main_def.is_none() && guard.is_none() {
        return;
    }
    let lineno = main_def.or(guard).map(Stmt::lineno).unwrap_or(0);
    let handler = main_def.and_then(|n| match n {
        Stmt::FunctionDef { name, .. } => Some(name.as_str()),
        _ => None,
    });
    raw.push(named_raw("script", "CLI", Some(module), handler, module, path, lineno));
    frameworks.insert("script".to_string());
}

/// Port of `kl4a.codekb.entrypoints._finalize`.
///
/// ```python
/// def _finalize(raw: list[dict], symbol_ids: set[str]) -> list[dict]:
///     endpoints: list[dict] = []; seen: dict[str, int] = {}
///     for e in raw:
///         if e["kind"] == "http" and e["framework"] == "flask":
///             full = _join_path(e["mount_prefix"], e["route_path"])
///             if e["path_resolved"] and not full: full = "/"
///             path_display = full if e["path_resolved"] else "(dynamic)"
///         else:
///             path_display = e["route_path"] or "(dynamic)"
///         qname = e["handler_qname"]
///         handler_sid = code_symbol_id_for(qname) if qname else None
///         handler_present = handler_sid in symbol_ids if handler_sid else False
///         base = bounded_id("endpoint", f"{e['framework']}-{e['method']}-{path_display}-{e['handler'] or e['name'] or ''}", max_length=120)
///         n = seen.get(base, 0); seen[base] = n + 1
///         eid = base if n == 0 else f"{base}-{n}"
///         endpoints.append({"id": eid, "kind": e["kind"], "framework": e["framework"], "method": e["method"],
///             "path": path_display, "route_path": e["route_path"], "router_prefix": e["router_prefix"],
///             "mount_prefix": e["mount_prefix"], "router_var": e["router_var"], "handler": e["handler"],
///             "handler_qname": qname, "handler_symbol_id": handler_sid if handler_present else None,
///             "module": e["module"], "file": e["file"], "line": e["line"], "status_code": None, "tags": [],
///             "summary": None, "name": e["name"], "path_resolved": e["path_resolved"]})
///     endpoints.sort(key=lambda x: (x["framework"], x["path"], x["method"]))
///     return endpoints
/// ```
fn finalize(raw: Vec<Value>, symbol_ids: &HashSet<String>) -> Vec<Value> {
    let mut endpoints: Vec<Value> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    for e in raw {
        let kind = e.get("kind").and_then(Value::as_str).unwrap_or_default().to_string();
        let framework = e.get("framework").and_then(Value::as_str).unwrap_or_default().to_string();
        let method = e.get("method").and_then(Value::as_str).unwrap_or_default().to_string();
        let route_path = e.get("route_path").and_then(Value::as_str).unwrap_or_default().to_string();
        let mount_prefix = e.get("mount_prefix").and_then(Value::as_str).unwrap_or_default().to_string();
        let path_resolved = e.get("path_resolved").and_then(Value::as_bool).unwrap_or(false);

        let path_display = if kind == "http" && framework == "flask" {
            let mut full = join_path(&[mount_prefix.as_str(), route_path.as_str()]);
            if path_resolved && full.is_empty() {
                full = "/".to_string();
            }
            if path_resolved {
                full
            } else {
                "(dynamic)".to_string()
            }
        } else if route_path.is_empty() {
            "(dynamic)".to_string()
        } else {
            route_path.clone()
        };

        let qname = e.get("handler_qname").cloned().unwrap_or(Value::Null);
        let handler_sid = qname.as_str().map(code_symbol_id_for);
        let handler_present = handler_sid.as_deref().map(|sid| symbol_ids.contains(sid)).unwrap_or(false);

        let handler = e.get("handler").and_then(Value::as_str).unwrap_or_default();
        let name = e.get("name").and_then(Value::as_str).unwrap_or_default();
        let handler_or_name = if !handler.is_empty() { handler } else { name };
        let base_source = format!("{framework}-{method}-{path_display}-{handler_or_name}");
        let base = bounded_id("endpoint", &base_source, 120);
        let n = *seen.get(&base).unwrap_or(&0);
        seen.insert(base.clone(), n + 1);
        let eid = if n == 0 { base.clone() } else { format!("{base}-{n}") };

        endpoints.push(json!({
            "id": eid,
            "kind": kind,
            "framework": framework,
            "method": method,
            "path": path_display,
            "route_path": route_path,
            "router_prefix": e.get("router_prefix").cloned().unwrap_or(Value::Null),
            "mount_prefix": mount_prefix,
            "router_var": e.get("router_var").cloned().unwrap_or(Value::Null),
            "handler": e.get("handler").cloned().unwrap_or(Value::Null),
            "handler_qname": qname,
            "handler_symbol_id": if handler_present { Value::String(handler_sid.unwrap()) } else { Value::Null },
            "module": e.get("module").cloned().unwrap_or(Value::Null),
            "file": e.get("file").cloned().unwrap_or(Value::Null),
            "line": e.get("line").cloned().unwrap_or(Value::Null),
            "status_code": Value::Null,
            "tags": Value::Array(vec![]),
            "summary": Value::Null,
            "name": e.get("name").cloned().unwrap_or(Value::Null),
            "path_resolved": path_resolved,
        }));
    }

    endpoints.sort_by(|a, b| {
        let key = |v: &Value| -> (String, String, String) {
            (
                v.get("framework").and_then(Value::as_str).unwrap_or_default().to_string(),
                v.get("path").and_then(Value::as_str).unwrap_or_default().to_string(),
                v.get("method").and_then(Value::as_str).unwrap_or_default().to_string(),
            )
        };
        key(a).cmp(&key(b))
    });
    endpoints
}

/// Port of `kl4a.codekb.entrypoints._iter_python_asts`.
///
/// ```python
/// def _iter_python_asts(bundle_dir: Path, sources: list[dict]) -> Iterator[tuple[str, str, ast.Module]]:
///     for src in sources:
///         if src.get("language") != "python": continue
///         original = bundle_dir / src.get("original_path", "")
///         if not original.exists(): continue
///         try:
///             tree = ast.parse(original.read_text(encoding="utf-8", errors="replace"))
///         except SyntaxError:
///             continue
///         yield module_name_for(src["path"]), src["path"], tree
/// ```
///
/// Now implemented for real via `rustpython_parser`, translating the result
/// into this file's local [`pyast::Module`] mirror (see the module-level
/// doc comment for why the mirror was kept rather than dropped).
/// `errors="replace"`'s lossy UTF-8 decoding is mirrored via
/// `String::from_utf8_lossy` over raw bytes (`std::fs::read_to_string`
/// would instead hard-fail on invalid UTF-8, which Python's
/// `read_text(..., errors="replace")` never does). A `SyntaxError` (here,
/// any `rustpython_parser` parse error) is skipped exactly like Python's
/// `except SyntaxError: continue`.
fn iter_python_asts(bundle_dir: &Path, sources: &[Value]) -> Vec<(String, String, Module)> {
    let mut out = Vec::new();
    for src in sources {
        if src.get("language").and_then(Value::as_str) != Some("python") {
            continue;
        }
        let original_path = src.get("original_path").and_then(Value::as_str).unwrap_or_default();
        let original = bundle_dir.join(original_path);
        if !original.exists() {
            continue;
        }
        let Ok(bytes) = std::fs::read(&original) else { continue };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let Ok(parsed) = rustpython_parser::parse(&text, Mode::Module, original_path) else { continue };
        let rpy::Mod::Module(module_ast) = parsed else { continue };
        let Some(path) = src.get("path").and_then(Value::as_str) else { continue };
        let li = LineIndex::new(&text);
        out.push((module_name_for(path), path.to_string(), translate_module(&module_ast, &li)));
    }
    out
}

/// Port of `kl4a.codekb.entrypoints.collect_entrypoints` (the module's sole
/// `__all__` export).
///
/// ```python
/// def collect_entrypoints(bundle_dir: Path, sources: list[dict], symbol_ids: set[str]) -> tuple[list[dict], set[str], set[str]]:
///     """Collect non-FastAPI entrypoints across all Python sources.
///     Returns (endpoints, frameworks, flask_router_vars)."""
///     asts = list(_iter_python_asts(bundle_dir, sources))
///     flask_apps: dict[str, dict] = {}; flask_blueprints: dict[str, dict] = {}; register_prefixes: dict[str, str] = {}
///     for module, path, tree in asts:
///         _scan_flask_objects(tree, module, flask_apps, flask_blueprints, register_prefixes)
///     flask_vars = set(flask_apps) | set(flask_blueprints)
///     raw: list[dict] = []; frameworks: set[str] = set()
///     for module, path, tree in asts:
///         before = len(raw)
///         _collect_flask(tree, module, path, flask_apps, flask_blueprints, register_prefixes, raw, frameworks)
///         _collect_django(tree, module, path, raw, frameworks)
///         _collect_click(tree, module, path, raw, frameworks)
///         _collect_argparse(tree, module, path, raw, frameworks)
///         _collect_celery(tree, module, path, raw, frameworks)
///         if len(raw) == before:
///             _collect_plain_script(tree, module, path, raw, frameworks)
///     endpoints = _finalize(raw, symbol_ids)
///     return endpoints, frameworks, flask_vars
/// ```
///
/// Returns `(module_name_for's` documented use is internal to
/// `iter_python_asts`, not part of this function's own contract)
/// `(endpoints, frameworks, flask_router_vars)`. `module`/`path` in `sources`
/// are `serde_json::Value`s (mirroring `dict[str, Any]`); `symbol_ids` is
/// the caller-supplied set of already-known symbol ids used to decide
/// whether a detected handler resolves to a real symbol.
pub fn collect_entrypoints(
    bundle_dir: &Path,
    sources: &[Value],
    symbol_ids: &HashSet<String>,
) -> (Vec<Value>, HashSet<String>, HashSet<String>) {
    let asts = iter_python_asts(bundle_dir, sources);

    let mut flask_apps: HashMap<String, FlaskApp> = HashMap::new();
    let mut flask_blueprints: HashMap<String, FlaskBlueprint> = HashMap::new();
    let mut register_prefixes: HashMap<String, String> = HashMap::new();
    for (module, _path, tree) in &asts {
        scan_flask_objects(tree, module, &mut flask_apps, &mut flask_blueprints, &mut register_prefixes);
    }
    let flask_vars: HashSet<String> = flask_apps.keys().cloned().chain(flask_blueprints.keys().cloned()).collect();

    let mut raw: Vec<Value> = Vec::new();
    let mut frameworks: HashSet<String> = HashSet::new();
    for (module, path, tree) in &asts {
        let before = raw.len();
        collect_flask(tree, module, path, &flask_apps, &flask_blueprints, &register_prefixes, &mut raw, &mut frameworks);
        collect_django(tree, module, path, &mut raw, &mut frameworks);
        collect_click(tree, module, path, &mut raw, &mut frameworks);
        collect_argparse(tree, module, path, &mut raw, &mut frameworks);
        collect_celery(tree, module, path, &mut raw, &mut frameworks);
        if raw.len() == before {
            collect_plain_script(tree, module, path, &mut raw, &mut frameworks);
        }
    }

    let endpoints = finalize(raw, symbol_ids);
    (endpoints, frameworks, flask_vars)
}
