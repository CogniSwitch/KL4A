//! Port of `kl4a/codekb/parse.py`.
//!
//! MCP evidence (`parse_python_bundle`, lines 44-94 of the Python source)
//! shows this module calling Python's real `ast.parse` on each inventoried
//! Python source file, then walking the resulting tree with `ast.unparse`,
//! `ast.get_docstring`, `node.lineno`/`node.end_lineno`, etc. — i.e. it
//! parses *arbitrary target-repo Python source*, not just structured
//! JSON/YAML.
//!
//! [`pyast`] below is this crate's own generic Python-AST representation
//! (`Node`/`Kind`), built specifically to support what `parse.py` and
//! `relations.py` (a sibling batch file, same port) actually need:
//! module-level imports, class/function/constant symbol structure with
//! signatures/decorators/docstrings, and a *generic* statement/expression
//! tree that a caller can walk uniformly (mirroring `ast.walk` /
//! `ast.iter_child_nodes`) to find `Call`/`Raise` nodes anywhere in a
//! symbol's own body — which `relations.py`'s `_walk_own_scope` depends on.
//! It now gets built by converting `rustpython_parser`'s real Python-grammar
//! AST into this shape (previously it was built by a from-scratch
//! hand-written tokenizer/recursive-descent parser; see git history / the
//! coordinator's Step-4 ledger for that prior version). **No public
//! function signature in this module changed** — `pyast::parse`,
//! `pyast::collect_symbol_nodes`, `pyast::walk_own_scope`, and every
//! `pub fn` below (`parse_python_bundle`, `symbol_records`, `module_record`,
//! `signature_for`, `decorators_for`, `import_names`, ...) are exactly as
//! before, so `relations.rs` and every other caller needed **zero** changes.
//!
//! Where `ast.unparse` normalizes formatting (base-class expressions,
//! decorator expressions, assignment/annotation values, a function's
//! parameter list), this port now reconstructs that text by recursing over
//! the real, structured AST (see `pyast`'s internal `convert_expr`/
//! `unparse_constant`/`unparse_arguments`) instead of slicing verbatim
//! source text — resolving the fidelity gap the previous hand-rolled parser
//! flagged as its main `// UNCONFIRMED:` risk. `pyast` still isn't a
//! byte-perfect `ast.unparse` for every exotic expression shape (see
//! `convert_expr`'s fallback arm), but every shape `signature_for`/
//! `decorators_for`/class-bases/`ann_fields` actually need is covered.

use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};

use crate::ids::{code_evidence_id_for, code_module_id_for, code_symbol_id_for};
use crate::okf_writer::write_markdown;
use crate::state::{read_json, write_code_state, STATE_DIR};

pub mod pyast {
    //! Converts `rustpython_parser`'s real Python-grammar AST into this
    //! crate's own generic `Node`/`Kind` shape, sufficient for
    //! `parse.rs`/`relations.rs`'s own needs. See the module-level doc
    //! comment on `crate::parse` for scope.
    //!
    //! ## API-surface caveats (flag for the coordinator's `cargo build` pass)
    //!
    //! Written against `rustpython_parser` 0.4.x's ast shapes from memory
    //! (no network access to crates.io/docs.rs to verify field names against
    //! the exact pinned version — see the handback report). The specific
    //! points most likely to need a field-name/path tweak once this
    //! actually compiles:
    //! - The top-level parse entry point: assumed to be
    //!   `rustpython_parser::parse(source, Mode::Module, "<module>") ->
    //!   Result<ast::Mod, ParseError>`.
    //! - `Ranged::range()` (imported as `rustpython_parser::ast::Ranged`) is
    //!   assumed to be how every node exposes its byte `TextRange`, and
    //!   `TextSize -> usize` is assumed to go through `u32::from(size)`.
    //! - `ast::Arguments`'s parameter-list shape (`posonlyargs`/`args`/
    //!   `vararg`/`kwonlyargs`/`kwarg`, each `ArgWithDefault` exposing its
    //!   inner `Arg` — assumed as a `.def_` field, since `def` is a Rust
    //!   keyword) is the least certain part of this port; see
    //!   `unparse_arg_with_default` below for the one line to fix.
    //! - `ast::StmtImportFrom.level`'s exact wrapper type (assumed
    //!   `Option<ast::Int>`, read via `u32::from(..)`).
    //! - `ast::Stmt::TryStar` may not exist in the pinned version's Python
    //!   grammar support (added late upstream) — safe to delete that match
    //!   arm if so.
    //!
    //! None of these affect this module's *behavior* once field names are
    //! corrected — they are isolated to the small conversion functions
    //! below rather than spread across the file.

    use rustpython_parser::ast::{self as ast, Ranged};
    use rustpython_parser::Mode;

    /// What kind of Python statement or expression a [`Node`] represents.
    /// Unlike a fully-typed `ast.AST` hierarchy, every kind shares the same
    /// [`Node`] shape so a caller can walk the tree generically (mirroring
    /// `ast.iter_child_nodes`) without a giant match over node types.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Kind {
        Module,
        Import,
        ImportFrom,
        ClassDef,
        FunctionDef,
        AsyncFunctionDef,
        Assign,
        AnnAssign,
        AugAssign,
        Return,
        Delete,
        Pass,
        Break,
        Continue,
        Global,
        Nonlocal,
        Assert,
        Raise,
        If,
        For,
        AsyncFor,
        While,
        With,
        AsyncWith,
        Try,
        ExceptHandler,
        ExprStmt,
        // expression kinds
        Name,
        Attribute,
        Call,
        Constant,
        BinOp,
        BoolOp,
        UnaryOp,
        Compare,
        IfExp,
        Lambda,
        ListExpr,
        TupleExpr,
        DictExpr,
        SetExpr,
        Subscript,
        Starred,
        Slice,
        Await,
        Yield,
        YieldFrom,
        JoinedStr,
        NamedExpr,
        Comprehension,
        Unknown,
    }

    /// A generic Python AST node. Every statement and expression kind this
    /// parser produces uses this same struct; irrelevant fields for a given
    /// `kind` are left at their default (`None`/empty). `children` holds
    /// **every** child node (sub-statements and sub-expressions alike),
    /// which is what makes a generic `ast.walk`-style traversal possible —
    /// see [`walk_own_scope`].
    #[derive(Debug, Clone, Default)]
    pub struct Node {
        pub kind_slot: Option<Kind>,
        pub lineno: usize,
        pub end_lineno: usize,
        /// `Name.id`, `Attribute.attr`, a `def`/`class` name, an import's
        /// bound name, etc. — whichever single identifier is this node's
        /// "own name", if any.
        pub name: Option<String>,
        /// Verbatim source text for this node's span. Used as the
        /// `ast.unparse` stand-in (see module docs for the fidelity
        /// caveat).
        pub text: String,
        /// Set only for a string-literal `Constant` (used for docstring
        /// extraction and for `env_read_name_for`/`file_access_for`-style
        /// literal argument inspection).
        pub string_value: Option<String>,
        /// All child nodes, generically. Mirrors `ast.iter_child_nodes`.
        pub children: Vec<Node>,
        /// `ClassDef`/`FunctionDef`/`AsyncFunctionDef` decorator
        /// expressions, in source order (also present in `children`).
        pub decorator_list: Vec<Node>,
        /// `ClassDef` base-class expressions (also present in `children`).
        pub bases: Vec<Node>,
        /// Nested statement body (also present in `children`).
        pub body: Vec<Node>,
        /// `Assign` targets / `AnnAssign` target (single element).
        pub targets: Vec<Node>,
        /// `Assign`/`AnnAssign` right-hand value, when present.
        pub value: Option<Box<Node>>,
        /// `AnnAssign` annotation expression.
        pub annotation: Option<Box<Node>>,
        /// `FunctionDef`/`AsyncFunctionDef` verbatim parenthesized
        /// parameter-list text (e.g. `"self, x: int = 1"`), used to
        /// reconstruct a signature without a full `arguments` model.
        pub args_text: String,
        /// `FunctionDef`/`AsyncFunctionDef` return annotation, if any.
        pub returns: Option<Box<Node>>,
        /// `Call.func`.
        pub call_func: Option<Box<Node>>,
        /// `Call.args` (positional).
        pub call_args: Vec<Node>,
        /// `Call.keywords`, as `(name, value)`; `name` is empty for a bare
        /// `**kwargs` spread (not otherwise distinguished — not needed by
        /// any current caller).
        pub call_keywords: Vec<(String, Node)>,
        /// `Raise.exc`, if any (a bare `raise` re-raises and has none).
        pub exc: Option<Box<Node>>,
        /// `Import`/`ImportFrom` names, as `(dotted_name, as_name)`.
        pub import_names: Vec<(String, Option<String>)>,
        /// `ImportFrom.module` (may be empty for a pure-relative `from . import x`).
        pub module: Option<String>,
        /// `ImportFrom.level` (count of leading dots).
        pub level: usize,
    }

    impl Node {
        fn new(kind: Kind, lineno: usize) -> Self {
            Node {
                kind_slot: Some(kind),
                lineno,
                end_lineno: lineno,
                ..Default::default()
            }
        }

        pub fn kind(&self) -> Kind {
            self.kind_slot.unwrap_or(Kind::Unknown)
        }

        pub fn is_def(&self) -> bool {
            matches!(self.kind(), Kind::FunctionDef | Kind::AsyncFunctionDef | Kind::ClassDef)
        }
    }

    // ---------------------------------------------------------------
    // Byte-offset -> 1-based line number. `rustpython_parser`'s AST nodes
    // carry byte `TextRange`s (via the `Ranged` trait), not pre-computed
    // line numbers, so every conversion function below goes through this.
    // ---------------------------------------------------------------

    struct LineIndex {
        /// `starts[i]` is the byte offset where line `i + 1` begins.
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

    /// The `(start_line, end_line)` pair for any `Ranged` node, 1-based and
    /// inclusive on both ends (matching `ast.lineno`/`ast.end_lineno`).
    fn span<T: Ranged>(li: &LineIndex, node: &T) -> (usize, usize) {
        let r = node.range();
        let start = li.at(u32::from(r.start()) as usize);
        let end = li.at((u32::from(r.end()) as usize).saturating_sub(1));
        (start, end)
    }

    // ---------------------------------------------------------------
    // Statement conversion
    // ---------------------------------------------------------------

    fn convert_stmts(stmts: &[ast::Stmt], li: &LineIndex) -> Vec<Node> {
        stmts.iter().map(|s| convert_stmt(s, li)).collect()
    }

    fn convert_stmt(stmt: &ast::Stmt, li: &LineIndex) -> Node {
        match stmt {
            ast::Stmt::FunctionDef(f) => {
                let (start, end) = span(li, f);
                convert_funcdef(f.name.as_str(), &f.args, &f.body, &f.decorator_list, f.returns.as_deref(), false, li, start, end)
            }
            ast::Stmt::AsyncFunctionDef(f) => {
                let (start, end) = span(li, f);
                convert_funcdef(f.name.as_str(), &f.args, &f.body, &f.decorator_list, f.returns.as_deref(), true, li, start, end)
            }
            ast::Stmt::ClassDef(c) => {
                let (start, end) = span(li, c);
                let mut node = Node::new(Kind::ClassDef, start);
                node.end_lineno = end;
                node.name = Some(c.name.to_string());
                let decorators: Vec<Node> = c.decorator_list.iter().map(|d| convert_expr(d, li)).collect();
                let bases: Vec<Node> = c.bases.iter().map(|b| convert_expr(b, li)).collect();
                let body = convert_stmts(&c.body, li);
                node.decorator_list = decorators.clone();
                node.bases = bases.clone();
                node.body = body.clone();
                node.children = decorators.into_iter().chain(bases).chain(body).collect();
                node
            }
            ast::Stmt::Return(r) => {
                let (start, end) = span(li, r);
                let mut node = Node::new(Kind::Return, start);
                node.end_lineno = end;
                if let Some(v) = &r.value {
                    let value_node = convert_expr(v, li);
                    node.children = vec![value_node.clone()];
                    node.value = Some(Box::new(value_node));
                }
                node
            }
            ast::Stmt::Delete(d) => {
                let (start, end) = span(li, d);
                let mut node = Node::new(Kind::Delete, start);
                node.end_lineno = end;
                node.children = d.targets.iter().map(|t| convert_expr(t, li)).collect();
                node
            }
            ast::Stmt::Assign(a) => {
                let (start, end) = span(li, a);
                let mut node = Node::new(Kind::Assign, start);
                node.end_lineno = end;
                let targets: Vec<Node> = a.targets.iter().map(|t| convert_expr(t, li)).collect();
                let value = convert_expr(&a.value, li);
                node.targets = targets.clone();
                node.value = Some(Box::new(value.clone()));
                node.children = targets.into_iter().chain(std::iter::once(value)).collect();
                node
            }
            ast::Stmt::AugAssign(a) => {
                let (start, end) = span(li, a);
                let mut node = Node::new(Kind::AugAssign, start);
                node.end_lineno = end;
                let target = convert_expr(&a.target, li);
                let value = convert_expr(&a.value, li);
                node.targets = vec![target.clone()];
                node.value = Some(Box::new(value.clone()));
                node.children = vec![target, value];
                node
            }
            ast::Stmt::AnnAssign(a) => {
                let (start, end) = span(li, a);
                let mut node = Node::new(Kind::AnnAssign, start);
                node.end_lineno = end;
                let target = convert_expr(&a.target, li);
                let annotation = convert_expr(&a.annotation, li);
                let value = a.value.as_deref().map(|v| convert_expr(v, li));
                node.targets = vec![target.clone()];
                node.annotation = Some(Box::new(annotation.clone()));
                node.value = value.clone().map(Box::new);
                let mut children = vec![target, annotation];
                if let Some(v) = value {
                    children.push(v);
                }
                node.children = children;
                node
            }
            ast::Stmt::For(f) => convert_for(&f.target, &f.iter, &f.body, &f.orelse, li, span(li, f), false),
            ast::Stmt::AsyncFor(f) => convert_for(&f.target, &f.iter, &f.body, &f.orelse, li, span(li, f), true),
            ast::Stmt::While(w) => {
                let (start, end) = span(li, w);
                let mut node = Node::new(Kind::While, start);
                node.end_lineno = end;
                let test = convert_expr(&w.test, li);
                let body = convert_stmts(&w.body, li);
                let orelse = convert_stmts(&w.orelse, li);
                node.value = Some(Box::new(test.clone()));
                node.body = body.clone();
                node.children = std::iter::once(test).chain(body).chain(orelse).collect();
                node
            }
            ast::Stmt::If(i) => {
                let (start, end) = span(li, i);
                let mut node = Node::new(Kind::If, start);
                node.end_lineno = end;
                let test = convert_expr(&i.test, li);
                let body = convert_stmts(&i.body, li);
                let orelse = convert_stmts(&i.orelse, li);
                node.value = Some(Box::new(test.clone()));
                node.body = body.clone();
                node.children = std::iter::once(test).chain(body).chain(orelse).collect();
                node
            }
            ast::Stmt::With(w) => {
                let (start, end) = span(li, w);
                convert_with(&w.items, &w.body, li, start, end, false)
            }
            ast::Stmt::AsyncWith(w) => {
                let (start, end) = span(li, w);
                convert_with(&w.items, &w.body, li, start, end, true)
            }
            ast::Stmt::Match(m) => {
                // Not in the original hand-rolled parser's grammar at all
                // (its own docs explicitly disclaimed `match` support);
                // best-effort generic container here so a `Call`/`Raise`
                // inside any case body is still reachable by
                // `walk_own_scope` — a genuine fidelity improvement now
                // that a real parser is available.
                let (start, end) = span(li, m);
                let mut node = Node::new(Kind::Unknown, start);
                node.end_lineno = end;
                let subject = convert_expr(&m.subject, li);
                let mut children = vec![subject];
                for case in &m.cases {
                    if let Some(guard) = &case.guard {
                        children.push(convert_expr(guard, li));
                    }
                    children.extend(convert_stmts(&case.body, li));
                }
                node.children = children;
                node
            }
            ast::Stmt::Raise(r) => {
                let (start, end) = span(li, r);
                let mut node = Node::new(Kind::Raise, start);
                node.end_lineno = end;
                if let Some(exc) = &r.exc {
                    let exc_node = convert_expr(exc, li);
                    let mut children = vec![exc_node.clone()];
                    if let Some(cause) = &r.cause {
                        children.push(convert_expr(cause, li));
                    }
                    node.exc = Some(Box::new(exc_node));
                    node.children = children;
                }
                node
            }
            ast::Stmt::Try(t) => convert_try(&t.body, &t.handlers, &t.orelse, &t.finalbody, li, span(li, t)),
            ast::Stmt::TryStar(t) => convert_try(&t.body, &t.handlers, &t.orelse, &t.finalbody, li, span(li, t)),
            ast::Stmt::Assert(a) => {
                let (start, end) = span(li, a);
                let mut node = Node::new(Kind::Assert, start);
                node.end_lineno = end;
                let mut children = vec![convert_expr(&a.test, li)];
                if let Some(msg) = &a.msg {
                    children.push(convert_expr(msg, li));
                }
                node.children = children;
                node
            }
            ast::Stmt::Import(i) => {
                let (start, end) = span(li, i);
                let mut node = Node::new(Kind::Import, start);
                node.end_lineno = end;
                node.import_names = i
                    .names
                    .iter()
                    .map(|a| (a.name.to_string(), a.asname.as_ref().map(|s| s.to_string())))
                    .collect();
                node
            }
            ast::Stmt::ImportFrom(i) => {
                let (start, end) = span(li, i);
                let mut node = Node::new(Kind::ImportFrom, start);
                node.end_lineno = end;
                node.module = Some(i.module.as_ref().map(|m| m.to_string()).unwrap_or_default());
                node.level = i.level.map(|l| l.to_usize()).unwrap_or(0);
                node.import_names = i
                    .names
                    .iter()
                    .map(|a| (a.name.to_string(), a.asname.as_ref().map(|s| s.to_string())))
                    .collect();
                node
            }
            ast::Stmt::Global(g) => {
                let (start, end) = span(li, g);
                let mut node = Node::new(Kind::Global, start);
                node.end_lineno = end;
                node.import_names = g.names.iter().map(|n| (n.to_string(), None)).collect();
                node
            }
            ast::Stmt::Nonlocal(n) => {
                let (start, end) = span(li, n);
                let mut node = Node::new(Kind::Nonlocal, start);
                node.end_lineno = end;
                node
            }
            ast::Stmt::Expr(e) => {
                let (start, end) = span(li, e);
                let mut node = Node::new(Kind::ExprStmt, start);
                node.end_lineno = end;
                let value = convert_expr(&e.value, li);
                node.string_value = value.string_value.clone();
                node.children = vec![value.clone()];
                node.value = Some(Box::new(value));
                node
            }
            ast::Stmt::Pass(p) => {
                let (start, end) = span(li, p);
                let mut node = Node::new(Kind::Pass, start);
                node.end_lineno = end;
                node
            }
            ast::Stmt::Break(b) => {
                let (start, end) = span(li, b);
                let mut node = Node::new(Kind::Break, start);
                node.end_lineno = end;
                node
            }
            ast::Stmt::Continue(c) => {
                let (start, end) = span(li, c);
                let mut node = Node::new(Kind::Continue, start);
                node.end_lineno = end;
                node
            }
            // Python 3.12's `type X = ...` statement (PEP 695). Not part of
            // the Python surface any codebase this tool has analyzed to date
            // uses, and `Kind` has no dedicated variant for it; modeled as a
            // no-op `Pass` node (matching this file's fallback pattern for
            // "syntax that parses but carries no symbol/relation meaning")
            // rather than silently dropping the statement or panicking.
            ast::Stmt::TypeAlias(t) => {
                let (start, end) = span(li, t);
                let mut node = Node::new(Kind::Pass, start);
                node.end_lineno = end;
                node
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn convert_funcdef(
        name: &str,
        args: &ast::Arguments,
        body: &[ast::Stmt],
        decorator_list: &[ast::Expr],
        returns: Option<&ast::Expr>,
        is_async: bool,
        li: &LineIndex,
        start_line: usize,
        end_line: usize,
    ) -> Node {
        let kind = if is_async { Kind::AsyncFunctionDef } else { Kind::FunctionDef };
        let mut node = Node::new(kind, start_line);
        node.end_lineno = end_line;
        node.name = Some(name.to_string());
        node.args_text = unparse_arguments(args, li);
        let decorators: Vec<Node> = decorator_list.iter().map(|d| convert_expr(d, li)).collect();
        let returns_node = returns.map(|r| convert_expr(r, li));
        let body_nodes = convert_stmts(body, li);
        node.returns = returns_node.clone().map(Box::new);
        node.decorator_list = decorators.clone();
        node.body = body_nodes.clone();

        // Fidelity note vs. the previous hand-rolled parser: real
        // `ast.walk` also descends into a function's parameter
        // defaults/annotations (genuine child fields of `FunctionDef.args`
        // in the real AST); the old hand-rolled parser only ever stored
        // `args_text` as opaque text and never surfaced those
        // sub-expressions to `walk_own_scope`, silently missing e.g. a
        // `Call` inside a default value. Now that real structured
        // `Arguments` are available, include them for full `ast.walk`
        // fidelity.
        let mut arg_exprs: Vec<Node> = Vec::new();
        for a in args.posonlyargs.iter().chain(args.args.iter()).chain(args.kwonlyargs.iter()) {
            if let Some(ann) = &a.def.annotation {
                arg_exprs.push(convert_expr(ann, li));
            }
            if let Some(default) = &a.default {
                arg_exprs.push(convert_expr(default, li));
            }
        }
        for a in [&args.vararg, &args.kwarg].into_iter().flatten() {
            if let Some(ann) = &a.annotation {
                arg_exprs.push(convert_expr(ann, li));
            }
        }

        let mut children: Vec<Node> = decorators;
        if let Some(r) = returns_node {
            children.push(r);
        }
        children.extend(arg_exprs);
        children.extend(body_nodes);
        node.children = children;
        node
    }

    fn convert_for(
        target: &ast::Expr,
        iter: &ast::Expr,
        body: &[ast::Stmt],
        orelse: &[ast::Stmt],
        li: &LineIndex,
        span: (usize, usize),
        is_async: bool,
    ) -> Node {
        let (start, end) = span;
        let mut node = Node::new(if is_async { Kind::AsyncFor } else { Kind::For }, start);
        node.end_lineno = end;
        let target_node = convert_expr(target, li);
        let iter_node = convert_expr(iter, li);
        node.name = Some(target_node.text.clone());
        node.value = Some(Box::new(iter_node.clone()));
        let body_nodes = convert_stmts(body, li);
        let orelse_nodes = convert_stmts(orelse, li);
        node.body = body_nodes.clone();
        node.children = vec![target_node, iter_node].into_iter().chain(body_nodes).chain(orelse_nodes).collect();
        node
    }

    fn convert_with(items: &[ast::WithItem], body: &[ast::Stmt], li: &LineIndex, start: usize, end: usize, is_async: bool) -> Node {
        let mut node = Node::new(if is_async { Kind::AsyncWith } else { Kind::With }, start);
        node.end_lineno = end;
        let mut item_children = Vec::new();
        for item in items {
            item_children.push(convert_expr(&item.context_expr, li));
            if let Some(vars) = &item.optional_vars {
                item_children.push(convert_expr(vars, li));
            }
        }
        let body_nodes = convert_stmts(body, li);
        node.body = body_nodes.clone();
        node.children = item_children.into_iter().chain(body_nodes).collect();
        node
    }

    fn convert_try(
        body: &[ast::Stmt],
        handlers: &[ast::ExceptHandler],
        orelse: &[ast::Stmt],
        finalbody: &[ast::Stmt],
        li: &LineIndex,
        span: (usize, usize),
    ) -> Node {
        let (start, end) = span;
        let mut node = Node::new(Kind::Try, start);
        node.end_lineno = end;
        let body_nodes = convert_stmts(body, li);
        let handler_nodes: Vec<Node> = handlers.iter().map(|h| convert_except_handler(h, li)).collect();
        let orelse_nodes = convert_stmts(orelse, li);
        let final_nodes = convert_stmts(finalbody, li);
        node.body = body_nodes.clone();
        node.children = body_nodes.into_iter().chain(handler_nodes).chain(orelse_nodes).chain(final_nodes).collect();
        node
    }

    fn convert_except_handler(h: &ast::ExceptHandler, li: &LineIndex) -> Node {
        let ast::ExceptHandler::ExceptHandler(eh) = h;
        let (start, end) = span(li, eh);
        let mut node = Node::new(Kind::ExceptHandler, start);
        node.end_lineno = end;
        let mut children = Vec::new();
        if let Some(t) = &eh.type_ {
            children.push(convert_expr(t, li));
        }
        let body_nodes = convert_stmts(&eh.body, li);
        node.body = body_nodes.clone();
        children.extend(body_nodes);
        node.children = children;
        node
    }

    // ---------------------------------------------------------------
    // Expression conversion — also builds `Node.text`, this port's
    // `ast.unparse` stand-in, bottom-up from each already-converted child's
    // own `.text` (see the module-level doc comment).
    // ---------------------------------------------------------------

    fn convert_expr(expr: &ast::Expr, li: &LineIndex) -> Node {
        match expr {
            ast::Expr::Name(n) => {
                let (start, end) = span(li, n);
                let mut node = Node::new(Kind::Name, start);
                node.end_lineno = end;
                node.name = Some(n.id.to_string());
                node.text = n.id.to_string();
                node
            }
            ast::Expr::Attribute(a) => {
                let (start, end) = span(li, a);
                let value = convert_expr(&a.value, li);
                let mut node = Node::new(Kind::Attribute, start);
                node.end_lineno = end;
                node.name = Some(a.attr.to_string());
                node.text = format!("{}.{}", parenthesized_base_text(&value), a.attr.as_str());
                node.children = vec![value];
                node
            }
            ast::Expr::Call(c) => {
                let (start, end) = span(li, c);
                let func = convert_expr(&c.func, li);
                let args: Vec<Node> = c.args.iter().map(|a| convert_expr(a, li)).collect();
                let keywords: Vec<(String, Node)> = c
                    .keywords
                    .iter()
                    .map(|k| (k.arg.as_ref().map(|s| s.to_string()).unwrap_or_default(), convert_expr(&k.value, li)))
                    .collect();
                let args_text = args
                    .iter()
                    .map(|a| a.text.clone())
                    .chain(keywords.iter().map(|(name, v)| {
                        if name.is_empty() { format!("**{}", v.text) } else { format!("{name}={}", v.text) }
                    }))
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut node = Node::new(Kind::Call, start);
                node.end_lineno = end;
                node.text = format!("{}({})", parenthesized_base_text(&func), args_text);
                node.call_func = Some(Box::new(func.clone()));
                node.call_args = args.clone();
                node.call_keywords = keywords.clone();
                let mut children = vec![func];
                children.extend(args);
                children.extend(keywords.into_iter().map(|(_, v)| v));
                node.children = children;
                node
            }
            ast::Expr::Constant(c) => {
                let (start, end) = span(li, c);
                let mut node = Node::new(Kind::Constant, start);
                node.end_lineno = end;
                node.text = unparse_constant(&c.value);
                if let ast::Constant::Str(s) = &c.value {
                    node.string_value = Some(s.clone());
                }
                node
            }
            ast::Expr::BinOp(b) => {
                let (start, end) = span(li, b);
                let left = convert_expr(&b.left, li);
                let right = convert_expr(&b.right, li);
                let mut node = Node::new(Kind::BinOp, start);
                node.end_lineno = end;
                node.text = format!("{} {} {}", left.text, binop_symbol(&b.op), right.text);
                node.children = vec![left, right];
                node
            }
            ast::Expr::BoolOp(b) => {
                let (start, end) = span(li, b);
                let values: Vec<Node> = b.values.iter().map(|v| convert_expr(v, li)).collect();
                let sep = match b.op {
                    ast::BoolOp::And => " and ",
                    ast::BoolOp::Or => " or ",
                };
                let mut node = Node::new(Kind::BoolOp, start);
                node.end_lineno = end;
                node.text = values.iter().map(|v| v.text.clone()).collect::<Vec<_>>().join(sep);
                node.children = values;
                node
            }
            ast::Expr::UnaryOp(u) => {
                let (start, end) = span(li, u);
                let operand = convert_expr(&u.operand, li);
                let sym = match u.op {
                    ast::UnaryOp::Invert => "~",
                    ast::UnaryOp::Not => "not ",
                    ast::UnaryOp::UAdd => "+",
                    ast::UnaryOp::USub => "-",
                };
                let mut node = Node::new(Kind::UnaryOp, start);
                node.end_lineno = end;
                node.text = format!("{sym}{}", operand.text);
                node.children = vec![operand];
                node
            }
            ast::Expr::Compare(c) => {
                let (start, end) = span(li, c);
                let left = convert_expr(&c.left, li);
                let comparators: Vec<Node> = c.comparators.iter().map(|e| convert_expr(e, li)).collect();
                let mut text = left.text.clone();
                for (op, comp) in c.ops.iter().zip(comparators.iter()) {
                    text.push_str(&format!(" {} {}", cmpop_symbol(op), comp.text));
                }
                let mut node = Node::new(Kind::Compare, start);
                node.end_lineno = end;
                node.text = text;
                node.children = std::iter::once(left).chain(comparators).collect();
                node
            }
            ast::Expr::IfExp(i) => {
                let (start, end) = span(li, i);
                let body = convert_expr(&i.body, li);
                let test = convert_expr(&i.test, li);
                let orelse = convert_expr(&i.orelse, li);
                let mut node = Node::new(Kind::IfExp, start);
                node.end_lineno = end;
                node.text = format!("{} if {} else {}", body.text, test.text, orelse.text);
                node.children = vec![body, test, orelse];
                node
            }
            ast::Expr::Lambda(l) => {
                // `ast.unparse` keeps the parameter list
                // (`lambda x, y=1: x + y`), which the previous version here
                // dropped entirely (always rendered `lambda: <body>`) — a
                // flagged fidelity gap. Confirmed against CPython:
                // `ast.unparse(ast.parse('lambda x, y=1: x+y').body[0].value)`
                // == `'lambda x, y=1: x + y'`.
                let (start, end) = span(li, l);
                let body = convert_expr(&l.body, li);
                let args_text = unparse_arguments(&l.args, li);
                let mut node = Node::new(Kind::Lambda, start);
                node.end_lineno = end;
                node.text = if args_text.is_empty() {
                    format!("lambda: {}", body.text)
                } else {
                    format!("lambda {args_text}: {}", body.text)
                };
                node.children = vec![body];
                node
            }
            ast::Expr::List(l) => {
                let (start, end) = span(li, l);
                let elts: Vec<Node> = l.elts.iter().map(|e| convert_expr(e, li)).collect();
                let mut node = Node::new(Kind::ListExpr, start);
                node.end_lineno = end;
                node.text = format!("[{}]", elts.iter().map(|e| e.text.clone()).collect::<Vec<_>>().join(", "));
                node.children = elts;
                node
            }
            ast::Expr::Tuple(t) => {
                let (start, end) = span(li, t);
                let elts: Vec<Node> = t.elts.iter().map(|e| convert_expr(e, li)).collect();
                let mut node = Node::new(Kind::TupleExpr, start);
                node.end_lineno = end;
                let joined = elts.iter().map(|e| e.text.clone()).collect::<Vec<_>>().join(", ");
                node.text = if elts.len() == 1 { format!("({joined},)") } else { format!("({joined})") };
                node.children = elts;
                node
            }
            ast::Expr::Set(s) => {
                let (start, end) = span(li, s);
                let elts: Vec<Node> = s.elts.iter().map(|e| convert_expr(e, li)).collect();
                let mut node = Node::new(Kind::SetExpr, start);
                node.end_lineno = end;
                node.text = format!("{{{}}}", elts.iter().map(|e| e.text.clone()).collect::<Vec<_>>().join(", "));
                node.children = elts;
                node
            }
            ast::Expr::Dict(d) => {
                let (start, end) = span(li, d);
                let mut children = Vec::new();
                let mut parts = Vec::new();
                for (k, v) in d.keys.iter().zip(d.values.iter()) {
                    let value_node = convert_expr(v, li);
                    match k {
                        Some(key_expr) => {
                            let key_node = convert_expr(key_expr, li);
                            parts.push(format!("{}: {}", key_node.text, value_node.text));
                            children.push(key_node);
                        }
                        None => {
                            parts.push(format!("**{}", value_node.text));
                        }
                    }
                    children.push(value_node);
                }
                let mut node = Node::new(Kind::DictExpr, start);
                node.end_lineno = end;
                node.text = format!("{{{}}}", parts.join(", "));
                node.children = children;
                node
            }
            ast::Expr::Subscript(s) => {
                let (start, end) = span(li, s);
                let value = convert_expr(&s.value, li);
                let slice = convert_expr(&s.slice, li);
                // `ast.unparse` omits the outer parens around a tuple slice
                // (`dict[str, str]`, not `dict[(str, str)]`), while still
                // keeping the single-element trailing comma
                // (`Tuple[int,]`) since that's syntactically load-bearing.
                // Confirmed against CPython: `ast.unparse(ast.parse(
                // "x: dict[str, str] = {}").body[0].annotation)` ==
                // `'dict[str, str]'`.
                let slice_text = bare_tuple_text(&s.slice, &slice);
                let mut node = Node::new(Kind::Subscript, start);
                node.end_lineno = end;
                node.text = format!("{}[{}]", parenthesized_base_text(&value), slice_text);
                node.children = vec![value, slice];
                node
            }
            ast::Expr::Starred(s) => {
                let (start, end) = span(li, s);
                let value = convert_expr(&s.value, li);
                let mut node = Node::new(Kind::Starred, start);
                node.end_lineno = end;
                node.text = format!("*{}", value.text);
                node.children = vec![value];
                node
            }
            ast::Expr::Slice(s) => {
                let (start, end) = span(li, s);
                let lower = s.lower.as_deref().map(|e| convert_expr(e, li));
                let upper = s.upper.as_deref().map(|e| convert_expr(e, li));
                let step = s.step.as_deref().map(|e| convert_expr(e, li));
                let mut node = Node::new(Kind::Slice, start);
                node.end_lineno = end;
                let mut text = format!(
                    "{}:{}",
                    lower.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
                    upper.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
                );
                if let Some(step_node) = &step {
                    text.push_str(&format!(":{}", step_node.text));
                }
                node.text = text;
                node.children = [lower, upper, step].into_iter().flatten().collect();
                node
            }
            ast::Expr::Await(a) => {
                let (start, end) = span(li, a);
                let value = convert_expr(&a.value, li);
                let mut node = Node::new(Kind::Await, start);
                node.end_lineno = end;
                node.text = format!("await {}", value.text);
                node.children = vec![value];
                node
            }
            ast::Expr::Yield(y) => {
                let (start, end) = span(li, y);
                let value = y.value.as_deref().map(|e| convert_expr(e, li));
                let mut node = Node::new(Kind::Yield, start);
                node.end_lineno = end;
                node.text = match &value {
                    Some(v) => format!("yield {}", v.text),
                    None => "yield".to_string(),
                };
                node.children = value.into_iter().collect();
                node
            }
            ast::Expr::YieldFrom(y) => {
                let (start, end) = span(li, y);
                let value = convert_expr(&y.value, li);
                let mut node = Node::new(Kind::YieldFrom, start);
                node.end_lineno = end;
                node.text = format!("yield from {}", value.text);
                node.children = vec![value];
                node
            }
            ast::Expr::NamedExpr(n) => {
                let (start, end) = span(li, n);
                let target = convert_expr(&n.target, li);
                let value = convert_expr(&n.value, li);
                let mut node = Node::new(Kind::NamedExpr, start);
                node.end_lineno = end;
                node.text = format!("{} := {}", target.text, value.text);
                node.children = vec![target, value];
                node
            }
            ast::Expr::JoinedStr(j) => {
                // Bug confirmed via a full `tools/kl4a` self-build diff:
                // this crate's own `CODE_REQUIRED_DIRS = [..., f"{STATE_DIR}/cache", ...]`
                // module constant unparsed as `f"{STATE_DIR}/cache"` here
                // but Python's `ast.unparse` gives `f'{STATE_DIR}/cache'`
                // (single-quoted). `ast.unparse` re-serializes from the AST
                // — it does not preserve the source's original quote
                // character — and an f-string follows the exact same
                // quote-choice rule as a plain string constant (prefer `'`,
                // switch to `"` only if the content contains `'` and no
                // `"`), confirmed directly: `ast.unparse` of
                // `f"{a}/indexes"` and of `f'{a}/indexes'` both give
                // `f'{a}/indexes'`; of `f"it's {a}"` (content has an
                // apostrophe) gives `f"it's {a}"` instead. The previous
                // version here always hard-coded `"`. Also now carries a
                // `!r`/`!s`/`!a` conversion and a `:`-format-spec through
                // (previously silently dropped), via [`fstring_body_text`].
                let (start, end) = span(li, j);
                let literal_concat = fstring_literal_concat(&j.values);
                let quote = choose_repr_quote(&literal_concat);
                let mut children = Vec::new();
                let body = fstring_body_text(&j.values, quote, li, &mut children);
                let text = format!("f{quote}{body}{quote}");
                let mut node = Node::new(Kind::JoinedStr, start);
                node.end_lineno = end;
                node.text = text;
                node.children = children;
                node
            }
            // Reached only if a `FormattedValue` shows up outside a
            // `JoinedStr` (shouldn't happen in valid Python, but handled so
            // this match stays exhaustive); just surfaces the inner value.
            ast::Expr::FormattedValue(fv) => convert_expr(&fv.value, li),
            ast::Expr::ListComp(c) => convert_comprehension(&[c.elt.as_ref()], &c.generators, li, span(li, c), "[", "]", ", "),
            ast::Expr::SetComp(c) => convert_comprehension(&[c.elt.as_ref()], &c.generators, li, span(li, c), "{", "}", ", "),
            ast::Expr::GeneratorExp(c) => convert_comprehension(&[c.elt.as_ref()], &c.generators, li, span(li, c), "(", ")", ", "),
            ast::Expr::DictComp(c) => convert_comprehension(&[c.key.as_ref(), c.value.as_ref()], &c.generators, li, span(li, c), "{", "}", ": "),
        }
    }

    /// Renders `expr`'s already-converted `node.text`, except when `expr` is
    /// itself a `Tuple` — then renders the elements joined by `", "` without
    /// the enclosing parens (`ast.unparse`'s "bare tuple" contexts: a
    /// subscript slice and a `for` loop/comprehension target). The
    /// single-element trailing comma is kept since it's syntactically
    /// meaningful (`Tuple[int,]`). Confirmed against CPython:
    /// `ast.unparse` of `dict[str, str]` is `'dict[str, str]'` (no parens),
    /// of `Tuple[int,]` is `'Tuple[int,]'` (comma kept), and of
    /// `{k: v for k, v in items}` is `'{k: v for k, v in items}'`.
    fn bare_tuple_text(raw: &ast::Expr, converted: &Node) -> String {
        match raw {
            ast::Expr::Tuple(t) => {
                let parts: Vec<String> = converted.children.iter().map(|c| c.text.clone()).collect();
                if t.elts.len() == 1 {
                    format!("{},", parts.join(", "))
                } else {
                    parts.join(", ")
                }
            }
            _ => converted.text.clone(),
        }
    }

    /// Real bug confirmed via a full `tools/kl4a` self-build diff (this
    /// crate's own `COBOL_REPO = (Path(__file__).parent / "fixtures" /
    /// "cobol_simple_repo").resolve()` module-level constant, among
    /// others): `ast.unparse` wraps a lower-precedence sub-expression in
    /// parens when it becomes the base of an `Attribute`/`Call`/
    /// `Subscript` — e.g. `(a / b).resolve()` stays parenthesized, not
    /// `a / b.resolve()` (which would wrongly read as `.resolve()` called
    /// on `b` alone). The previous version here always concatenated
    /// `value.text` directly with no such wrapping, so a parenthesized
    /// `BinOp`/`BoolOp`/etc. used as a call/attribute/subscript base lost
    /// its grouping — silently changing what the printed signature text
    /// actually means, not just its formatting. Confirmed field-by-field
    /// against CPython for every `Kind` below (`ast.unparse` of
    /// `(a / b).resolve()`, `(a or b).x`, `(not a).x`, `(lambda: 1)()`,
    /// `(a if b else c).x`, `(yield a).x`, `(a := 1).x`, `(await a).x`,
    /// `(a < b).x` all keep their parens; `a.b.c()`, `a[0].b`, `f(x).y`,
    /// `(a, b).count(1)`, `[1, 2].count(1)`, `{1: 2}.get(1)`, `a().b`
    /// — Name/Attribute/Call/Subscript/Tuple/List/Dict bases — do not).
    fn needs_parens_as_base(kind: Kind) -> bool {
        matches!(
            kind,
            Kind::BinOp
                | Kind::BoolOp
                | Kind::UnaryOp
                | Kind::Compare
                | Kind::IfExp
                | Kind::Lambda
                | Kind::NamedExpr
                | Kind::Yield
                | Kind::YieldFrom
                | Kind::Await
        )
    }

    /// Applies [`needs_parens_as_base`] to a converted node's own `.text`.
    fn parenthesized_base_text(node: &Node) -> String {
        if needs_parens_as_base(node.kind()) {
            format!("({})", node.text)
        } else {
            node.text.clone()
        }
    }

    /// Builds the real `for <target> in <iter> [if <cond>]...` clause text
    /// for one comprehension generator, matching `ast.unparse`.
    fn comprehension_clause_text(gen: &ast::Comprehension, li: &LineIndex) -> String {
        let target_node = convert_expr(&gen.target, li);
        let target_text = bare_tuple_text(&gen.target, &target_node);
        let iter_text = convert_expr(&gen.iter, li).text;
        let mut clause = if gen.is_async {
            format!(" async for {target_text} in {iter_text}")
        } else {
            format!(" for {target_text} in {iter_text}")
        };
        for cond in &gen.ifs {
            clause.push_str(&format!(" if {}", convert_expr(cond, li).text));
        }
        clause
    }

    /// Full `ast.unparse` reconstruction for a list/set/generator/dict
    /// comprehension (previously rendered as an opaque `"<comprehension>"`
    /// placeholder — a flagged fidelity gap, since this text can appear in
    /// a symbol's `signature` field, e.g. `DEFAULT = [i for i in range(10)]`).
    fn convert_comprehension(elts: &[&ast::Expr], generators: &[ast::Comprehension], li: &LineIndex, span: (usize, usize), open: &str, close: &str, sep: &str) -> Node {
        let (start, end) = span;
        let elt_nodes: Vec<Node> = elts.iter().map(|e| convert_expr(e, li)).collect();
        let elt_text = elt_nodes.iter().map(|n| n.text.clone()).collect::<Vec<_>>().join(sep);
        let mut text = format!("{open}{elt_text}");
        for gen in generators {
            text.push_str(&comprehension_clause_text(gen, li));
        }
        text.push_str(close);
        let mut children: Vec<Node> = elt_nodes;
        for gen in generators {
            children.push(convert_expr(&gen.iter, li));
            for cond in &gen.ifs {
                children.push(convert_expr(cond, li));
            }
        }
        let mut node = Node::new(Kind::Comprehension, start);
        node.end_lineno = end;
        node.text = text;
        node.children = children;
        node
    }

    fn binop_symbol(op: &ast::Operator) -> &'static str {
        match op {
            ast::Operator::Add => "+",
            ast::Operator::Sub => "-",
            ast::Operator::Mult => "*",
            ast::Operator::MatMult => "@",
            ast::Operator::Div => "/",
            ast::Operator::Mod => "%",
            ast::Operator::Pow => "**",
            ast::Operator::LShift => "<<",
            ast::Operator::RShift => ">>",
            ast::Operator::BitOr => "|",
            ast::Operator::BitXor => "^",
            ast::Operator::BitAnd => "&",
            ast::Operator::FloorDiv => "//",
        }
    }

    fn cmpop_symbol(op: &ast::CmpOp) -> &'static str {
        match op {
            ast::CmpOp::Eq => "==",
            ast::CmpOp::NotEq => "!=",
            ast::CmpOp::Lt => "<",
            ast::CmpOp::LtE => "<=",
            ast::CmpOp::Gt => ">",
            ast::CmpOp::GtE => ">=",
            ast::CmpOp::Is => "is",
            ast::CmpOp::IsNot => "is not",
            ast::CmpOp::In => "in",
            ast::CmpOp::NotIn => "not in",
        }
    }

    /// `repr()`-style quoting for a constant, the way `ast.unparse` renders
    /// one (e.g. a string default value must round-trip as quoted Python
    /// source: `'abc'`, not bare `abc`).
    fn unparse_constant(c: &ast::Constant) -> String {
        match c {
            ast::Constant::None => "None".to_string(),
            ast::Constant::Bool(b) => if *b { "True".to_string() } else { "False".to_string() },
            ast::Constant::Str(s) => python_repr_str(s),
            ast::Constant::Bytes(b) => format!("b{}", python_repr_str(&String::from_utf8_lossy(b))),
            ast::Constant::Int(i) => i.to_string(),
            ast::Constant::Float(f) => format_python_float(*f),
            ast::Constant::Complex { real, imag } => format!("({real}+{imag}j)"),
            ast::Constant::Tuple(items) => {
                let joined = items.iter().map(unparse_constant).collect::<Vec<_>>().join(", ");
                if items.len() == 1 { format!("({joined},)") } else { format!("({joined})") }
            }
            ast::Constant::Ellipsis => "...".to_string(),
        }
    }

    /// Minimal `repr()`-style string quoting: prefers single quotes
    /// (matching `ast.unparse`'s default), falling back to double quotes
    /// when the text itself contains a single quote and no double quote —
    /// the same rule CPython's own `ast.unparse` follows for simple cases.
    /// Not a full `repr()` implementation (arbitrary control-character
    /// escaping is approximated, not exhaustive).
    fn python_repr_str(s: &str) -> String {
        let quote = choose_repr_quote(s);
        let mut out = String::with_capacity(s.len() + 2);
        out.push(quote);
        escape_for_quote(s, quote, &mut out);
        out.push(quote);
        out
    }

    /// `ast.unparse`'s quote-choice rule (factored out of
    /// [`python_repr_str`] so the same rule can also be applied to an
    /// f-string's literal text — see [`convert_expr`]'s `JoinedStr` arm):
    /// prefer `'`, switch to `"` only when the content contains a `'` and
    /// no `"`.
    fn choose_repr_quote(s: &str) -> char {
        if s.contains('\'') && !s.contains('"') {
            '"'
        } else {
            '\''
        }
    }

    /// The character-escaping half of [`python_repr_str`], factored out so
    /// it can be applied piecewise to an f-string's literal segments
    /// (interleaved with un-escaped `{expr}` substitutions) using one
    /// quote char chosen for the whole f-string up front.
    fn escape_for_quote(s: &str, quote: char, out: &mut String) {
        for ch in s.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c == quote => {
                    out.push('\\');
                    out.push(c);
                }
                c => out.push(c),
            }
        }
    }

    /// Concatenates only the literal (`Constant::Str`) segments of an
    /// f-string's `values`, ignoring `{expr}` substitutions — used to pick
    /// the whole f-string's quote character the way `ast.unparse` does
    /// (content-based, independent of source quoting).
    fn fstring_literal_concat(values: &[ast::Expr]) -> String {
        let mut s = String::new();
        for part in values {
            if let ast::Expr::Constant(c) = part {
                if let ast::Constant::Str(lit) = &c.value {
                    s.push_str(lit);
                }
            }
        }
        s
    }

    /// `!s`/`!r`/`!a`/no conversion, matching `ast.unparse`'s
    /// `FormattedValue.conversion` rendering.
    fn conversion_suffix(flag: ast::ConversionFlag) -> &'static str {
        match flag {
            ast::ConversionFlag::Str => "!s",
            ast::ConversionFlag::Repr => "!r",
            ast::ConversionFlag::Ascii => "!a",
            ast::ConversionFlag::None => "",
        }
    }

    /// Builds the unquoted body of an f-string (or a nested format-spec,
    /// which is itself represented as a `JoinedStr`) for the already-chosen
    /// `quote` character: literal segments are escaped for that quote
    /// (via [`escape_for_quote`]); `{expr}` substitutions carry their
    /// `!conversion` and `:format_spec` through, recursing for a nested
    /// format spec. Appends every substituted sub-expression's `Node` to
    /// `children` so `ast.walk`-style traversal still reaches them.
    fn fstring_body_text(values: &[ast::Expr], quote: char, li: &LineIndex, children: &mut Vec<Node>) -> String {
        let mut text = String::new();
        for part in values {
            match part {
                ast::Expr::Constant(c) => {
                    if let ast::Constant::Str(s) = &c.value {
                        escape_for_quote(s, quote, &mut text);
                    }
                }
                ast::Expr::FormattedValue(fv) => {
                    let inner = convert_expr(&fv.value, li);
                    text.push('{');
                    text.push_str(&inner.text);
                    text.push_str(conversion_suffix(fv.conversion));
                    if let Some(spec) = &fv.format_spec {
                        text.push(':');
                        if let ast::Expr::JoinedStr(nested) = spec.as_ref() {
                            text.push_str(&fstring_body_text(&nested.values, quote, li, children));
                        } else {
                            text.push_str(&convert_expr(spec, li).text);
                        }
                    }
                    text.push('}');
                    children.push(inner);
                }
                other => children.push(convert_expr(other, li)),
            }
        }
        text
    }

    /// `// UNCONFIRMED:` Python's `repr(float)`/`str(float)` formatting
    /// (e.g. always showing a decimal point: `1.0` not `1`) isn't
    /// guaranteed byte-identical to Rust's `f64::to_string` in every case
    /// (exponents, very large/small magnitudes). Good enough for the
    /// annotation/default values this codebase actually uses.
    fn format_python_float(f: f64) -> String {
        if f.is_finite() && f.fract() == 0.0 {
            format!("{f:.1}")
        } else {
            f.to_string()
        }
    }

    /// A genuine `ast.unparse(node.args)` equivalent, reconstructed from
    /// the real, structured `Arguments` — replacing the previous hand-rolled
    /// parser's verbatim-source-slice `args_text`, which was this port's
    /// main flagged fidelity gap (see `signature_for`'s grounded Python
    /// source: `ast.unparse(node.args)`).
    fn unparse_arguments(args: &ast::Arguments, li: &LineIndex) -> String {
        let mut parts: Vec<String> = Vec::new();
        for a in &args.posonlyargs {
            parts.push(unparse_arg_with_default(a, li));
        }
        if !args.posonlyargs.is_empty() {
            parts.push("/".to_string());
        }
        for a in &args.args {
            parts.push(unparse_arg_with_default(a, li));
        }
        if let Some(va) = &args.vararg {
            parts.push(format!("*{}", unparse_arg(va, li)));
        } else if !args.kwonlyargs.is_empty() {
            parts.push("*".to_string());
        }
        for a in &args.kwonlyargs {
            parts.push(unparse_arg_with_default(a, li));
        }
        if let Some(kw) = &args.kwarg {
            parts.push(format!("**{}", unparse_arg(kw, li)));
        }
        parts.join(", ")
    }

    fn unparse_arg(arg: &ast::Arg, li: &LineIndex) -> String {
        match &arg.annotation {
            Some(ann) => format!("{}: {}", arg.arg.as_str(), convert_expr(ann, li).text),
            None => arg.arg.to_string(),
        }
    }

    /// Confirmed against the pinned `rustpython-ast` 0.4.0 source
    /// (`ArgWithDefault.def: Arg<R>`) that the field is in fact named
    /// `def`, not `def_` — no fix needed there.
    ///
    /// Formatting fix: `ast.unparse` renders a parameter default with **no**
    /// spaces around `=` in both the unannotated and annotated cases
    /// (`b=1`, `c: int=2`) — confirmed against CPython:
    /// `ast.unparse(ast.parse('def f(a, b=1, *, c: int = 2): pass').body[0].args)`
    /// == `'a, b=1, *, c: int=2'`. The previous version here used
    /// `"{base} = {value}"` (spaced), which only matches Python's unrelated
    /// top-level-assignment signature text (`NAME = value`, built directly
    /// in `signature_for`, not via `ast.unparse`) — not a parameter default.
    fn unparse_arg_with_default(a: &ast::ArgWithDefault, li: &LineIndex) -> String {
        let base = unparse_arg(&a.def, li);
        match &a.default {
            Some(d) => format!("{base}={}", convert_expr(d, li).text),
            None => base,
        }
    }

    /// Parses a full module. Mirrors `ast.parse(text, filename=...)`,
    /// returning a `(line, message)` pair compatible with the shape
    /// `parse_python_bundle` records on a `SyntaxError`. The exact failing
    /// line isn't extracted from `rustpython_parser`'s `ParseError` (kept
    /// as `None`) to avoid depending on its exact internal shape; the
    /// message text is still recorded via `Display`.
    pub fn parse(source: &str) -> std::result::Result<Node, (Option<usize>, String)> {
        let parsed = rustpython_parser::parse(source, Mode::Module, "<module>").map_err(|e| (None, e.to_string()))?;
        let module = match parsed {
            ast::Mod::Module(m) => m,
            _ => return Err((None, "expected a module".to_string())),
        };
        let li = LineIndex::new(source);
        let body = convert_stmts(&module.body, &li);
        let mut node = Node::new(Kind::Module, 1);
        node.end_lineno = body.last().map(|n| n.end_lineno).unwrap_or(1);
        node.body = body.clone();
        node.children = body;
        Ok(node)
    }

    /// Port of `ast.iter_child_nodes` for our generic [`Node`]: every
    /// direct child, uniformly.
    pub fn iter_child_nodes(node: &Node) -> impl Iterator<Item = &Node> {
        node.children.iter()
    }

    /// Port of `kl4a.codekb.relations._collect_symbol_nodes` /
    /// `iter_symbol_nodes`'s traversal primitive, generalized: collects
    /// every `ClassDef`/`FunctionDef`/`AsyncFunctionDef`/`Assign`/
    /// `AnnAssign` node reachable from `body`, recursing into class and
    /// function bodies (this is the *symbol-collection* walk, distinct
    /// from [`walk_own_scope`] which is the *within-one-symbol* walk).
    pub fn collect_symbol_nodes<'a>(body: &'a [Node], out: &mut Vec<&'a Node>) {
        for node in body {
            match node.kind() {
                Kind::ClassDef => {
                    out.push(node);
                    collect_symbol_nodes(&node.body, out);
                }
                Kind::FunctionDef | Kind::AsyncFunctionDef => {
                    out.push(node);
                    collect_symbol_nodes(&node.body, out);
                }
                Kind::Assign | Kind::AnnAssign => {
                    out.push(node);
                }
                _ => {}
            }
        }
    }

    /// Port of `kl4a.codekb.relations._walk_own_scope`: like `ast.walk`
    /// starting at `node`, but does not descend into a nested
    /// `FunctionDef`/`AsyncFunctionDef`/`ClassDef`'s own subtree (that
    /// belongs to its own separate symbol). The root `node` itself is
    /// always visited even if it is itself a def/class.
    pub fn walk_own_scope(node: &Node) -> Vec<&Node> {
        let mut found = Vec::new();
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            if !std::ptr::eq(current, node) && current.is_def() {
                continue;
            }
            found.push(current);
            stack.extend(current.children.iter());
        }
        found
    }
}

use pyast::{Kind, Node};

/// Port of `kl4a.codekb.parse.parse_code_bundle`.
///
/// Dispatches to [`parse_python_bundle`] and (cross-batch)
/// `adapters::cobol_adapter::parse_cobol_bundle` depending on `language`
/// (`None` runs both, matching Python's `language in {None, "python"}` /
/// `{None, "cobol"}` checks), then merges + sorts the combined
/// modules/symbols/evidence/parser_runs and writes `code_symbols.json` +
/// `parser_runs.json`.
///
/// ## Cross-batch dependency (NOT part of this batch)
/// `crate::adapters::cobol_adapter::parse_cobol_bundle(bundle_dir) ->
/// Result<Value>` (a COBOL adapter, out of scope for this Python-focused
/// batch) — expected to return the same `{"modules", "symbols", "evidence",
/// "parser_runs"}` shape as [`parse_python_bundle`].
pub fn parse_code_bundle(bundle_dir: &Path, language: Option<&str>) -> Result<Value> {
    let mut results: Vec<Value> = Vec::new();
    if language.is_none() || language == Some("python") {
        results.push(parse_python_bundle(bundle_dir)?);
    }
    if language.is_none() || language == Some("cobol") {
        results.push(crate::adapters::cobol_adapter::parse_cobol_bundle(bundle_dir)?);
    }

    let mut modules: Vec<Value> = Vec::new();
    let mut symbols: Vec<Value> = Vec::new();
    let mut evidence: Vec<Value> = Vec::new();
    let mut parser_runs: Vec<Value> = Vec::new();
    for state in &results {
        modules.extend(state.get("modules").and_then(Value::as_array).cloned().unwrap_or_default());
        symbols.extend(state.get("symbols").and_then(Value::as_array).cloned().unwrap_or_default());
        evidence.extend(state.get("evidence").and_then(Value::as_array).cloned().unwrap_or_default());
        parser_runs.extend(state.get("parser_runs").and_then(Value::as_array).cloned().unwrap_or_default());
    }
    modules.sort_by(|a, b| sort_key(a, "qualified_name").cmp(&sort_key(b, "qualified_name")));
    symbols.sort_by(|a, b| sort_key(a, "qualified_name").cmp(&sort_key(b, "qualified_name")));
    evidence.sort_by(|a, b| sort_key(a, "id").cmp(&sort_key(b, "id")));

    let result = json!({
        "modules": modules,
        "symbols": symbols,
        "evidence": evidence,
        "parser_runs": parser_runs,
    });
    write_code_state(bundle_dir, "code_symbols.json", &result)?;
    write_code_state(bundle_dir, "parser_runs.json", &json!(parser_runs))?;
    Ok(result)
}

fn sort_key(value: &Value, field: &str) -> String {
    value.get(field).and_then(Value::as_str).unwrap_or("").to_string()
}

/// Port of `kl4a.codekb.parse.parse_python_bundle`.
///
/// Reads `code_inventory.json`, and for every `python`-language source,
/// reads the file, parses it (recording a `parse_error` parser-run entry
/// and skipping the source on a syntax error, matching Python's
/// `except SyntaxError` branch), then builds a module record + every
/// symbol/evidence record and writes each as an OKF markdown doc, exactly
/// mirroring the original's per-source loop order.
pub fn parse_python_bundle(bundle_dir: &Path) -> Result<Value> {
    let inventory = read_json(&bundle_dir.join(STATE_DIR).join("code_inventory.json"), json!({"sources": []}));
    let mut modules: Vec<Value> = Vec::new();
    let mut symbols: Vec<Value> = Vec::new();
    let mut evidence: Vec<Value> = Vec::new();
    let mut parser_runs: Vec<Value> = Vec::new();

    let sources = inventory.get("sources").and_then(Value::as_array).cloned().unwrap_or_default();
    for source in sources {
        if source.get("language").and_then(Value::as_str) != Some("python") {
            continue;
        }
        let original_path = source.get("original_path").and_then(Value::as_str).unwrap_or_default();
        let source_path = bundle_dir.join(original_path);
        let text = std::fs::read_to_string(&source_path)
            .map_err(|e| anyhow!("failed to read {}: {e}", source_path.display()))?;

        let mut run = Map::new();
        run.insert("source_id".into(), source.get("id").cloned().unwrap_or(Value::Null));
        run.insert(
            "source_version_id".into(),
            source.get("source_version_id").cloned().unwrap_or(Value::Null),
        );
        run.insert("parser".into(), json!("python-ast"));
        run.insert("parser_version".into(), json!("stdlib"));
        run.insert("status".into(), json!("parsed"));
        run.insert("errors".into(), json!([]));

        let tree = match pyast::parse(&text) {
            Ok(tree) => tree,
            Err((line, message)) => {
                run.insert("status".into(), json!("parse_error"));
                run["errors"] = json!([{"line": line, "message": message}]);
                parser_runs.push(Value::Object(run));
                continue;
            }
        };

        let module = module_record(&source, &tree);
        modules.push(module.clone());
        write_module_doc(bundle_dir, &module)?;

        for symbol in symbol_records(&source, &module, &tree, &text) {
            symbols.push(symbol.clone());
            let symbol_evidence = evidence_record(&source, &symbol, &text);
            evidence.push(symbol_evidence.clone());
            write_symbol_doc(bundle_dir, &symbol, &symbol_evidence)?;
            write_evidence_doc(bundle_dir, &symbol_evidence)?;
        }
        parser_runs.push(Value::Object(run));
    }

    let result = json!({
        "modules": modules,
        "symbols": symbols,
        "evidence": evidence,
        "parser_runs": parser_runs,
    });
    write_code_state(bundle_dir, "code_symbols.json", &result)?;
    write_code_state(bundle_dir, "parser_runs.json", &json!(parser_runs))?;
    Ok(result)
}

/// Port of `kl4a.codekb.parse.module_name_for`.
pub fn module_name_for(relative_path: &str) -> String {
    let path = Path::new(relative_path);
    let stem_path = path.with_extension("");
    let mut parts: Vec<String> = stem_path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    if parts.last().map(|s| s.as_str()) == Some("__init__") {
        parts.pop();
    }
    if parts.is_empty() {
        return stem_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
    }
    parts.join(".")
}

/// Port of `kl4a.codekb.parse.import_names`.
pub fn import_names(tree: &Node) -> Vec<String> {
    let mut imports: Vec<String> = Vec::new();
    for node in &tree.body {
        match node.kind() {
            Kind::Import => {
                for (dotted, _asname) in &node.import_names {
                    imports.push(dotted.clone());
                }
            }
            Kind::ImportFrom => {
                let module = format!("{}{}", ".".repeat(node.level), node.module.clone().unwrap_or_default());
                for (name, _asname) in &node.import_names {
                    let combined = format!("{module}.{name}");
                    imports.push(combined.trim_matches('.').to_string());
                }
            }
            _ => {}
        }
    }
    imports.sort();
    imports.dedup();
    imports
}

/// Port of `kl4a.codekb.parse.module_record`.
pub fn module_record(source: &Value, tree: &Node) -> Value {
    let path = source.get("path").and_then(Value::as_str).unwrap_or_default();
    let module_name = module_name_for(path);
    let imports = import_names(tree);
    let module_id = code_module_id_for(&module_name);
    json!({
        "id": module_id,
        "title": module_name,
        "qualified_name": module_name,
        "source_id": source.get("id").cloned().unwrap_or(Value::Null),
        "source_version_id": source.get("source_version_id").cloned().unwrap_or(Value::Null),
        "file": path,
        "language": "python",
        "imports": imports,
        "okf_path": format!("code/modules/{module_id}.md"),
    })
}

/// Port of `kl4a.codekb.parse._is_constant_name`.
///
/// A plain ALL_CAPS name, or the one dunder name (`__all__`) that's
/// conventionally treated as module metadata even though it has no cased
/// characters to satisfy Python's `.isupper()`.
fn is_constant_name(name: &str) -> bool {
    is_python_upper(name) || name == "__all__"
}

/// Mirrors Python `str.isupper()`: true iff there is at least one cased
/// character and all cased characters are uppercase.
fn is_python_upper(name: &str) -> bool {
    let mut has_cased = false;
    for c in name.chars() {
        if c.is_lowercase() {
            return false;
        }
        if c.is_uppercase() {
            has_cased = true;
        }
    }
    has_cased
}

/// Port of `kl4a.codekb.parse._collect_symbol_records`.
///
/// Recurses generically the same way the Python original does: a class
/// body recurses with `in_class=true`; a function body recurses under a
/// `<locals>`-style qualname with `in_class=false`, so two same-named
/// nested helpers in two different outer functions cannot collide.
fn collect_symbol_records(
    source: &Value,
    module: &Value,
    body: &[Node],
    parent: Option<&str>,
    in_class: bool,
    records: &mut Vec<Value>,
) {
    for node in body {
        match node.kind() {
            Kind::ClassDef => {
                records.push(symbol_record(source, module, node, "class", parent, None));
                let class_qual = join_qual(parent, node.name.as_deref());
                collect_symbol_records(source, module, &node.body, Some(&class_qual), true, records);
            }
            Kind::FunctionDef | Kind::AsyncFunctionDef => {
                let name = node.name.clone().unwrap_or_default();
                let kind = if in_class {
                    "method"
                } else if name.starts_with("test_") || source.get("kind").and_then(Value::as_str) == Some("test") {
                    "test"
                } else {
                    "function"
                };
                records.push(symbol_record(source, module, node, kind, parent, None));
                let func_qual = join_qual(parent, Some(&format!("{name}.<locals>")));
                collect_symbol_records(source, module, &node.body, Some(&func_qual), false, records);
            }
            Kind::Assign => {
                for target in &node.targets {
                    if target.kind() == Kind::Name {
                        if let Some(id) = &target.name {
                            if is_constant_name(id) {
                                records.push(symbol_record(source, module, node, "constant", parent, Some(id)));
                            }
                        }
                    }
                }
            }
            Kind::AnnAssign => {
                if let Some(target) = node.targets.first() {
                    if target.kind() == Kind::Name {
                        if let Some(id) = &target.name {
                            if is_constant_name(id) {
                                records.push(symbol_record(source, module, node, "constant", parent, Some(id)));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn join_qual(parent: Option<&str>, name: Option<&str>) -> String {
    [parent, name].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(".")
}

/// Port of `kl4a.codekb.parse.symbol_records`.
pub fn symbol_records(source: &Value, module: &Value, tree: &Node, _text: &str) -> Vec<Value> {
    let mut records = Vec::new();
    collect_symbol_records(source, module, &tree.body, None, false, &mut records);
    records
}

fn ast_node_type_name(kind: Kind) -> &'static str {
    match kind {
        Kind::ClassDef => "ClassDef",
        Kind::FunctionDef => "FunctionDef",
        Kind::AsyncFunctionDef => "AsyncFunctionDef",
        Kind::Assign => "Assign",
        Kind::AnnAssign => "AnnAssign",
        _ => "Unknown",
    }
}

/// Port of `kl4a.codekb.parse.symbol_record`.
pub fn symbol_record(
    source: &Value,
    module: &Value,
    node: &Node,
    kind: &str,
    parent: Option<&str>,
    name: Option<&str>,
) -> Value {
    let symbol_name = name.map(str::to_string).unwrap_or_else(|| node.name.clone().unwrap_or_default());
    let qualified_name = join_qual(
        Some(&join_qual(module.get("qualified_name").and_then(Value::as_str), parent)),
        Some(&symbol_name),
    );
    let symbol_id = code_symbol_id_for(&qualified_name);
    let signature = signature_for(node, &symbol_name);
    let decorators = decorators_for(node);
    let docstring = docstring_for(node);
    let evidence_id = code_evidence_id_for(&symbol_id);
    json!({
        "id": symbol_id,
        "title": symbol_name,
        "kind": kind,
        "qualified_name": qualified_name,
        "module_id": module.get("id").cloned().unwrap_or(Value::Null),
        "module": module.get("qualified_name").cloned().unwrap_or(Value::Null),
        "source_id": source.get("id").cloned().unwrap_or(Value::Null),
        "source_version_id": source.get("source_version_id").cloned().unwrap_or(Value::Null),
        "file": source.get("path").cloned().unwrap_or(Value::Null),
        "language": "python",
        "line_start": node.lineno,
        "line_end": node.end_lineno,
        "ast_node_type": ast_node_type_name(node.kind()),
        "signature": signature,
        "decorators": decorators,
        "docstring": docstring,
        "okf_path": format!("code/symbols/{symbol_id}.md"),
        "evidence_id": evidence_id,
    })
}

/// Port of `ast.get_docstring`'s cleaning step (`inspect.cleandoc`), which
/// the raw `string_value` extraction here previously skipped entirely —
/// a flagged fidelity gap (Medium finding: "Docstrings: Python uses
/// `ast.get_docstring` (which cleans/dedents); Rust currently takes the raw
/// string"). Confirmed against CPython's `inspect.cleandoc` algorithm:
/// expand tabs, strip the first line, find the minimum indentation among
/// all non-blank subsequent lines and strip that many columns from each,
/// then drop leading/trailing blank lines. Verified against
/// `ast.get_docstring` directly: for a docstring whose raw text is
/// `"\n    Summary line.\n\n        Indented detail.\n    Trailing.\n    "`,
/// `ast.get_docstring` returns
/// `"Summary line.\n\n    Indented detail.\nTrailing."`.
fn clean_doc(raw: &str) -> String {
    let expanded: Vec<String> = raw.split('\n').map(expand_tabs).collect();
    let mut margin = usize::MAX;
    for line in expanded.iter().skip(1) {
        let stripped = line.trim_start();
        if !stripped.is_empty() {
            let indent = line.len() - stripped.len();
            margin = margin.min(indent);
        }
    }
    let mut lines: Vec<String> = Vec::with_capacity(expanded.len());
    for (i, line) in expanded.iter().enumerate() {
        if i == 0 {
            lines.push(line.trim_start().to_string());
        } else if margin != usize::MAX {
            lines.push(line.chars().skip(margin).collect());
        } else {
            lines.push(line.clone());
        }
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    while lines.first().is_some_and(|l| l.is_empty()) {
        lines.remove(0);
    }
    lines.join("\n")
}

/// `str.expandtabs()` with Python's default tabsize of 8: each tab advances
/// to the next column that's a multiple of 8, counted in characters (not
/// bytes) from the start of the line, matching `inspect.cleandoc`'s own use
/// of `expandtabs()` before measuring indentation.
fn expand_tabs(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut col = 0usize;
    for ch in line.chars() {
        if ch == '\t' {
            let spaces = 8 - (col % 8);
            out.push_str(&" ".repeat(spaces));
            col += spaces;
        } else {
            out.push(ch);
            col += 1;
        }
    }
    out
}

fn docstring_for(node: &Node) -> Value {
    if !matches!(node.kind(), Kind::ClassDef | Kind::FunctionDef | Kind::AsyncFunctionDef) {
        return Value::Null;
    }
    match node.body.first() {
        Some(first) if first.kind() == Kind::ExprStmt => match &first.string_value {
            Some(s) => json!(clean_doc(s)),
            None => Value::Null,
        },
        _ => Value::Null,
    }
}

/// Port of `kl4a.codekb.parse.signature_for`.
///
/// Grounded via MCP against the exact Python source: values Python builds
/// via `ast.unparse` (`node.args`/`node.returns`/base classes/assignment
/// values/annotations) are now reconstructed from the real, structured AST
/// (see `pyast::convert_expr`/`unparse_arguments`), not a verbatim
/// source-text slice — resolving this port's previously flagged fidelity
/// gap.
pub fn signature_for(node: &Node, symbol_name: &str) -> Option<String> {
    match node.kind() {
        Kind::FunctionDef | Kind::AsyncFunctionDef => {
            let prefix = if node.kind() == Kind::AsyncFunctionDef { "async def" } else { "def" };
            let returns = node
                .returns
                .as_ref()
                .map(|r| format!(" -> {}", r.text_or_name()))
                .unwrap_or_default();
            Some(format!("{prefix} {symbol_name}({}){returns}", node.args_text))
        }
        Kind::ClassDef => {
            let bases: Vec<String> = node.bases.iter().map(|b| b.text_or_name()).collect();
            if bases.is_empty() {
                Some(format!("class {symbol_name}"))
            } else {
                Some(format!("class {symbol_name}({})", bases.join(", ")))
            }
        }
        Kind::Assign => node.value.as_ref().map(|v| format!("{symbol_name} = {}", v.text_or_name())),
        Kind::AnnAssign => {
            let annotation = node.annotation.as_ref().map(|a| a.text_or_name()).unwrap_or_default();
            match &node.value {
                Some(v) => Some(format!("{symbol_name}: {annotation} = {}", v.text_or_name())),
                None => Some(format!("{symbol_name}: {annotation}")),
            }
        }
        _ => None,
    }
}

impl Node {
    /// Best-effort `ast.unparse` stand-in: prefers the already-reconstructed
    /// `text` built during AST conversion (see `pyast::convert_expr`),
    /// falling back to its identifier (`Name`/`Attribute`) or empty text
    /// otherwise.
    pub fn text_or_name(&self) -> String {
        if !self.text.is_empty() {
            return self.text.clone();
        }
        if let Some(name) = &self.name {
            return name.clone();
        }
        match self.kind() {
            Kind::Attribute => {
                let base = self.children.first().map(|c| c.text_or_name()).unwrap_or_default();
                format!("{base}.{}", self.name.clone().unwrap_or_default())
            }
            _ => String::new(),
        }
    }
}

/// Port of `kl4a.codekb.parse.decorators_for`.
pub fn decorators_for(node: &Node) -> Vec<String> {
    if !matches!(node.kind(), Kind::ClassDef | Kind::FunctionDef | Kind::AsyncFunctionDef) {
        return Vec::new();
    }
    node.decorator_list.iter().map(|d| d.text_or_name()).collect()
}

/// Port of `kl4a.codekb.parse.evidence_record`.
pub fn evidence_record(source: &Value, symbol: &Value, text: &str) -> Value {
    let lines: Vec<&str> = text.lines().collect();
    let start = (symbol.get("line_start").and_then(Value::as_u64).unwrap_or(1) as usize).max(1);
    let end = (symbol.get("line_end").and_then(Value::as_u64).unwrap_or(start as u64) as usize).max(start);
    let excerpt = lines
        .get(start - 1..end.min(lines.len()))
        .map(|s| s.join("\n"))
        .unwrap_or_default();
    let evidence_id = symbol.get("evidence_id").cloned().unwrap_or(Value::Null);
    json!({
        "id": evidence_id,
        "title": format!("{} lines {start}-{end}", symbol.get("qualified_name").and_then(Value::as_str).unwrap_or("")),
        "source_id": source.get("id").cloned().unwrap_or(Value::Null),
        "source_version_id": source.get("source_version_id").cloned().unwrap_or(Value::Null),
        "symbol_id": symbol.get("id").cloned().unwrap_or(Value::Null),
        "file": source.get("path").cloned().unwrap_or(Value::Null),
        "language": "python",
        "line_start": start,
        "line_end": end,
        "ast_node_type": symbol.get("ast_node_type").cloned().unwrap_or(Value::Null),
        "span_status": "exact",
        "excerpt": excerpt,
        "okf_path": format!("evidence/{}.md", evidence_id.as_str().unwrap_or("")),
    })
}

/// Port of `kl4a.codekb.parse.write_module_doc`.
pub fn write_module_doc(bundle_dir: &Path, module: &Value) -> Result<()> {
    let okf_path = module.get("okf_path").and_then(Value::as_str).unwrap_or_default();
    let mut frontmatter = Map::new();
    frontmatter.insert("type".into(), json!("Code Module"));
    frontmatter.insert("title".into(), module.get("title").cloned().unwrap_or(Value::Null));
    frontmatter.insert("module_id".into(), module.get("id").cloned().unwrap_or(Value::Null));
    frontmatter.insert(
        "code".into(),
        json!({
            "language": module.get("language").cloned().unwrap_or(Value::Null),
            "qualified_name": module.get("qualified_name").cloned().unwrap_or(Value::Null),
            "file": module.get("file").cloned().unwrap_or(Value::Null),
            "imports": module.get("imports").cloned().unwrap_or_else(|| json!([])),
        }),
    );
    frontmatter.insert(
        "links".into(),
        json!({"source_file": format!("../../sources/files/{}.md", module.get("source_id").and_then(Value::as_str).unwrap_or(""))}),
    );
    let title = module.get("title").and_then(Value::as_str).unwrap_or("");
    write_markdown(&bundle_dir.join(okf_path), &Value::Object(frontmatter), &format!("# {title}\n"))
}

/// Port of `kl4a.codekb.parse.write_symbol_doc`.
pub fn write_symbol_doc(bundle_dir: &Path, symbol: &Value, evidence: &Value) -> Result<()> {
    let okf_path = symbol.get("okf_path").and_then(Value::as_str).unwrap_or_default();
    let evidence_id = evidence.get("id").cloned().unwrap_or(Value::Null);
    let mut frontmatter = Map::new();
    frontmatter.insert("type".into(), json!("Code Symbol"));
    frontmatter.insert("title".into(), symbol.get("title").cloned().unwrap_or(Value::Null));
    frontmatter.insert("symbol_id".into(), symbol.get("id").cloned().unwrap_or(Value::Null));
    frontmatter.insert(
        "code".into(),
        json!({
            "language": symbol.get("language").cloned().unwrap_or(Value::Null),
            "kind": symbol.get("kind").cloned().unwrap_or(Value::Null),
            "qualified_name": symbol.get("qualified_name").cloned().unwrap_or(Value::Null),
            "file": symbol.get("file").cloned().unwrap_or(Value::Null),
            "module": symbol.get("module").cloned().unwrap_or(Value::Null),
            "line_start": symbol.get("line_start").cloned().unwrap_or(Value::Null),
            "line_end": symbol.get("line_end").cloned().unwrap_or(Value::Null),
            "ast_node_type": symbol.get("ast_node_type").cloned().unwrap_or(Value::Null),
            "signature": symbol.get("signature").cloned().unwrap_or(Value::Null),
            "decorators": symbol.get("decorators").cloned().unwrap_or_else(|| json!([])),
            "docstring": symbol.get("docstring").cloned().unwrap_or(Value::Null),
        }),
    );
    frontmatter.insert("evidence".into(), json!([evidence_id]));
    frontmatter.insert(
        "links".into(),
        json!({
            "source_file": format!("../../sources/files/{}.md", symbol.get("source_id").and_then(Value::as_str).unwrap_or("")),
            "module": format!("../modules/{}.md", symbol.get("module_id").and_then(Value::as_str).unwrap_or("")),
            "evidence": format!("../../evidence/{}.md", evidence_id.as_str().unwrap_or("")),
        }),
    );
    let body = format!(
        "# {}\n\nQualified name: `{}`\n\nEvidence: [lines {}-{}](../../evidence/{}.md)\n",
        symbol.get("title").and_then(Value::as_str).unwrap_or(""),
        symbol.get("qualified_name").and_then(Value::as_str).unwrap_or(""),
        evidence.get("line_start").and_then(Value::as_u64).unwrap_or(0),
        evidence.get("line_end").and_then(Value::as_u64).unwrap_or(0),
        evidence_id.as_str().unwrap_or(""),
    );
    write_markdown(&bundle_dir.join(okf_path), &Value::Object(frontmatter), &body)
}

/// Port of `kl4a.codekb.parse.write_evidence_doc`.
pub fn write_evidence_doc(bundle_dir: &Path, evidence: &Value) -> Result<()> {
    let okf_path = evidence.get("okf_path").and_then(Value::as_str).unwrap_or_default();
    let mut frontmatter = Map::new();
    frontmatter.insert("type".into(), json!("Code Evidence"));
    frontmatter.insert("title".into(), evidence.get("title").cloned().unwrap_or(Value::Null));
    frontmatter.insert("evidence_id".into(), evidence.get("id").cloned().unwrap_or(Value::Null));
    frontmatter.insert(
        "code".into(),
        json!({
            "language": evidence.get("language").cloned().unwrap_or(Value::Null),
            "file": evidence.get("file").cloned().unwrap_or(Value::Null),
            "symbol_id": evidence.get("symbol_id").cloned().unwrap_or(Value::Null),
            "line_start": evidence.get("line_start").cloned().unwrap_or(Value::Null),
            "line_end": evidence.get("line_end").cloned().unwrap_or(Value::Null),
            "ast_node_type": evidence.get("ast_node_type").cloned().unwrap_or(Value::Null),
            "span_status": evidence.get("span_status").cloned().unwrap_or(Value::Null),
        }),
    );
    let body = format!(
        "# {}\n\n```python\n{}\n```\n",
        evidence.get("title").and_then(Value::as_str).unwrap_or(""),
        evidence.get("excerpt").and_then(Value::as_str).unwrap_or(""),
    );
    write_markdown(&bundle_dir.join(okf_path), &Value::Object(frontmatter), &body)
}

// Re-export so `relations.rs` (a sibling batch file) can reference the
// shared Python-AST machinery as `crate::parse::pyast`.
pub use pyast::{collect_symbol_nodes, walk_own_scope};
