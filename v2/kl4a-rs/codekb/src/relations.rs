//! Port of `kl4a/codekb/relations.py`.
//!
//! Static-analysis pass that derives code relations (`defines`, `imports`,
//! `inherits_from`, `decorated_by`, `calls`, `reads`/`writes` (environment
//! variables and files), `raises`, `covers`/`tested_by`) from the Python
//! ASTs parsed by [`crate::parse`], plus a generic-language pass-through for
//! non-Python relations (COBOL). Ported symbol-for-symbol from the Python
//! source (verified via tools-code MCP `code_symbols_get`).
//!
//! ## Cross-batch dependencies (NOT part of this batch — report only)
//!
//! - `crate::bundle_store::write_code_state`
//! - `crate::state::{read_json, STATE_DIR}`
//! - `crate::ids::code_relation_id_for`
//! - `crate::okf_writer::write_markdown`
//! - `crate::adapters::cobol_adapter::generate_cobol_relations(bundle_dir,
//!   symbols_state) -> Result<Vec<Value>>`

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use serde_json::{json, Map, Value};

use crate::ids::code_relation_id_for;
use crate::okf_writer::write_markdown;
use crate::parse::pyast::{self, Kind, Node};
use crate::state::{read_json, write_code_state, STATE_DIR};

/// Port of `kl4a.codekb.relations.SymbolAst`.
///
/// Python's version is a plain `@dataclass` holding a symbol record plus
/// the AST node matched to it; `node` is owned here (not borrowed) since
/// [`load_symbol_asts`] builds these from freshly-parsed, per-call trees.
#[derive(Debug, Clone)]
pub struct SymbolAst {
    pub symbol: Value,
    pub node: Node,
    pub source: Value,
    pub module: Value,
    pub text: String,
}

fn vstr(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}
fn vstr_opt(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}
fn vid(v: &Value) -> String {
    vstr(v, "id")
}

/// Port of `kl4a.codekb.relations.iter_symbol_nodes`.
fn iter_symbol_nodes(tree: &Node) -> Vec<&Node> {
    let mut nodes = Vec::new();
    pyast::collect_symbol_nodes(&tree.body, &mut nodes);
    nodes
}

/// Port of `kl4a.codekb.relations.load_symbol_asts`.
///
/// Skips a source that doesn't exist on disk or fails to parse, exactly as
/// Python's `if not source_path.exists(): continue` / `except SyntaxError:
/// continue` do. Matches each parsed AST node to its already-known symbol
/// record purely by `(source_id, line_start, line_end)`, taking the first
/// match (mirrors Python's `next((... for ... ), None)`).
pub fn load_symbol_asts(
    bundle_dir: &Path,
    sources_by_id: &HashMap<String, Value>,
    modules_by_id: &HashMap<String, Value>,
    symbol_by_id: &HashMap<String, Value>,
) -> Vec<SymbolAst> {
    let mut records = Vec::new();
    for source in sources_by_id.values() {
        if source.get("language").and_then(Value::as_str) != Some("python") {
            continue;
        }
        let original_path = vstr(source, "original_path");
        let source_path = bundle_dir.join(&original_path);
        if !source_path.exists() {
            continue;
        }
        let text = match std::fs::read_to_string(&source_path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let tree = match pyast::parse(&text) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let source_id = vid(source);
        let module = match modules_by_id.values().find(|m| vstr_opt(m, "source_id").as_deref() == Some(source_id.as_str())) {
            Some(m) => m.clone(),
            None => continue,
        };

        for node in iter_symbol_nodes(&tree) {
            let line_start = node.lineno;
            let line_end = node.end_lineno;
            let matched = symbol_by_id.values().find(|symbol| {
                vstr_opt(symbol, "source_id").as_deref() == Some(source_id.as_str())
                    && symbol.get("line_start").and_then(Value::as_u64) == Some(line_start as u64)
                    && symbol.get("line_end").and_then(Value::as_u64) == Some(line_end as u64)
            });
            if let Some(symbol) = matched {
                records.push(SymbolAst {
                    symbol: symbol.clone(),
                    node: node.clone(),
                    source: source.clone(),
                    module: module.clone(),
                    text: text.clone(),
                });
            }
        }
    }
    records
}

/// Port of `kl4a.codekb.relations.build_class_bases`.
///
/// Maps each class's `qualified_name` to its own (unresolved) base-class
/// name expressions, so a `self.foo`/`cls.foo` call not defined on the
/// calling method's own class can still be resolved by walking up to
/// whichever base class actually defines `foo`.
pub fn build_class_bases(ast_symbols: &[SymbolAst]) -> HashMap<String, Vec<String>> {
    let mut bases = HashMap::new();
    for ast_symbol in ast_symbols {
        if ast_symbol.symbol.get("kind").and_then(Value::as_str) == Some("class")
            && ast_symbol.node.kind() == Kind::ClassDef
        {
            let names: Vec<String> = ast_symbol.node.bases.iter().map(|b| b.text_or_name()).collect();
            bases.insert(vstr(&ast_symbol.symbol, "qualified_name"), names);
        }
    }
    bases
}

/// Port of `kl4a.codekb.relations.build_short_symbol_index`.
pub fn build_short_symbol_index(symbols: &[Value]) -> HashMap<String, Vec<Value>> {
    let mut index: HashMap<String, Vec<Value>> = HashMap::new();
    for symbol in symbols {
        let title = vstr(symbol, "title");
        let qualified_name = vstr(symbol, "qualified_name");
        let short = qualified_name.rsplit('.').next().unwrap_or(&qualified_name).to_string();
        let mut names: HashSet<String> = HashSet::new();
        names.insert(title);
        names.insert(short);
        for name in names {
            index.entry(name).or_default().push(symbol.clone());
        }
    }
    index
}

/// Port of `kl4a.codekb.relations.call_name_for`.
///
/// Recursively unwraps `Attribute`/`Call` nodes down to a dotted callable
/// name (`"os.environ.get"`, `"self.foo"`, ...), returning `None` for
/// anything else (e.g. a call on a subscript or literal).
pub fn call_name_for(node: &Node) -> Option<String> {
    match node.kind() {
        Kind::Name => node.name.clone(),
        Kind::Attribute => {
            let parent = node.children.first().and_then(call_name_for);
            let attr = node.name.clone().unwrap_or_default();
            Some(match parent {
                Some(p) => format!("{p}.{attr}"),
                None => attr,
            })
        }
        Kind::Call => node.call_func.as_deref().and_then(call_name_for),
        _ => None,
    }
}

/// Port of `kl4a.codekb.relations.env_read_name_for`.
pub fn env_read_name_for(node: &Node) -> Option<String> {
    let call_name = node.call_func.as_deref().and_then(call_name_for)?;
    if !matches!(call_name.as_str(), "os.environ.get" | "environ.get" | "os.getenv") {
        return None;
    }
    let first = node.call_args.first()?;
    if first.kind() == Kind::Constant {
        first.string_value.clone()
    } else {
        None
    }
}

/// Port of `kl4a.codekb.relations.exception_name_for`.
pub fn exception_name_for(node: &Node) -> Option<String> {
    let exc = node.exc.as_deref()?;
    match exc.kind() {
        Kind::Call => exc.call_func.as_deref().and_then(call_name_for),
        Kind::Name => exc.name.clone(),
        _ => Some(exc.text_or_name()),
    }
}

/// Port of `kl4a.codekb.relations.file_access_for`.
pub fn file_access_for(node: &Node) -> Option<(String, String, String)> {
    let call_name = node.call_func.as_deref().and_then(call_name_for).unwrap_or_default();
    if call_name == "open" || call_name.ends_with(".open") {
        let mut mode = "r".to_string();
        if let Some(second) = node.call_args.get(1) {
            if second.kind() == Kind::Constant {
                if let Some(s) = &second.string_value {
                    mode = s.clone();
                }
            }
        }
        for (name, value) in &node.call_keywords {
            if name == "mode" && value.kind() == Kind::Constant {
                if let Some(s) = &value.string_value {
                    mode = s.clone();
                }
            }
        }
        let predicate = if ["w", "a", "+"].iter().any(|flag| mode.contains(flag)) {
            "writes"
        } else {
            "reads"
        };
        return Some((predicate.to_string(), format!("file-open:{mode}"), "file_access".to_string()));
    }
    if call_name.ends_with(".read_text") || call_name.ends_with(".read_bytes") || call_name.ends_with(".glob") {
        let object_name = if call_name.ends_with(".glob") { "file-glob" } else { "file-read" };
        return Some(("reads".to_string(), object_name.to_string(), "file_access".to_string()));
    }
    if call_name.ends_with(".write_text") || call_name.ends_with(".write_bytes") {
        return Some(("writes".to_string(), "file-write".to_string(), "file_access".to_string()));
    }
    None
}

/// Port of `kl4a.codekb.relations.resolve_import_target`.
///
/// `import_names()` records `from X import Y` as `"X.Y"` — indistinguishable,
/// as a string, from a plain `import X.Y`. So `import_name` might already be
/// a full module path, or a module path with a trailing *symbol* name tacked
/// on. Tries it as-is first; if nothing matches, retries with the last
/// dotted segment dropped (treating it as the imported symbol's own name).
/// Matched by suffix, not equality, because a module's own `qualified_name`
/// is derived from its path relative to the scanned root, which commonly
/// carries a prefix the import string itself never includes.
pub fn resolve_import_target(import_name: &str, modules: &[Value], importing_module: &str) -> Option<Value> {
    fn candidates_for<'a>(name: &str, modules: &'a [Value]) -> Vec<&'a Value> {
        let exact: Vec<&Value> = modules.iter().filter(|m| vstr(m, "qualified_name") == name).collect();
        if !exact.is_empty() {
            return exact;
        }
        let suffix = format!(".{name}");
        modules.iter().filter(|m| vstr(m, "qualified_name").ends_with(&suffix)).collect()
    }
    fn pick(found: Vec<&Value>, importing_module: &str) -> Option<Value> {
        if found.len() == 1 {
            return Some(found[0].clone());
        }
        if found.len() > 1 {
            let scored: Vec<(usize, &Value)> = found
                .iter()
                .map(|m| (shared_prefix_len(importing_module, &vstr(m, "qualified_name")), *m))
                .collect();
            let best_score = scored.iter().map(|(s, _)| *s).max().unwrap_or(0);
            let best: Vec<&Value> = scored.into_iter().filter(|(s, _)| *s == best_score).map(|(_, m)| m).collect();
            if best.len() == 1 {
                return Some(best[0].clone());
            }
        }
        None
    }

    if let Some(target) = pick(candidates_for(import_name, modules), importing_module) {
        return Some(target);
    }
    if import_name.contains('.') {
        let trimmed = import_name.rsplit_once('.').map(|(head, _)| head).unwrap_or(import_name);
        return pick(candidates_for(trimmed, modules), importing_module);
    }
    None
}

/// Port of `kl4a.codekb.relations._shared_prefix_len`.
fn shared_prefix_len(module_a: &str, module_b: &str) -> usize {
    module_a
        .split('.')
        .zip(module_b.split('.'))
        .take_while(|(a, b)| a == b)
        .count()
}

/// Port of `kl4a.codekb.relations._members_of_class`.
fn members_of_class(candidates: &[Value], class_qname: &str) -> Vec<Value> {
    candidates
        .iter()
        .filter(|c| {
            let qname = vstr(c, "qualified_name");
            qname.rsplit_once('.').map(|(head, _)| head).unwrap_or(&qname) == class_qname
        })
        .cloned()
        .collect()
}

/// Port of `kl4a.codekb.relations._resolve_base_qname`.
///
/// Best-effort qualified name for a base-class expression like `Provider`
/// or `pkg.Base`, resolved against the set of classes this bundle actually
/// knows about. Prefers a class in the same module as the subclass, then an
/// exact match of the expression itself, then falls back to a unique
/// same-short-name class anywhere in the bundle.
fn resolve_base_qname(base_name: &str, owning_module: &str, class_bases: &HashMap<String, Vec<String>>) -> Option<String> {
    let same_module_guess = if owning_module.is_empty() {
        base_name.to_string()
    } else {
        format!("{owning_module}.{base_name}")
    };
    if class_bases.contains_key(&same_module_guess) {
        return Some(same_module_guess);
    }
    if class_bases.contains_key(base_name) {
        return Some(base_name.to_string());
    }
    let short = base_name.rsplit('.').next().unwrap_or(base_name);
    let matches: Vec<&String> = class_bases.keys().filter(|qname| qname.rsplit('.').next().unwrap_or(qname) == short).collect();
    if matches.len() == 1 {
        Some(matches[0].clone())
    } else {
        None
    }
}

/// Port of `kl4a.codekb.relations._members_of_class_hierarchy`.
fn members_of_class_hierarchy(
    candidates: &[Value],
    class_qname: &str,
    class_bases: &HashMap<String, Vec<String>>,
    seen: &mut HashSet<String>,
) -> Vec<Value> {
    if seen.contains(class_qname) || !class_bases.contains_key(class_qname) {
        return Vec::new();
    }
    seen.insert(class_qname.to_string());
    let owning_module = class_qname.rsplit_once('.').map(|(head, _)| head).unwrap_or("");
    for base_name in &class_bases[class_qname] {
        let Some(base_qname) = resolve_base_qname(base_name, owning_module, class_bases) else { continue };
        if seen.contains(&base_qname) {
            continue;
        }
        let direct = members_of_class(candidates, &base_qname);
        if !direct.is_empty() {
            return direct;
        }
        let deeper = members_of_class_hierarchy(candidates, &base_qname, class_bases, seen);
        if !deeper.is_empty() {
            return deeper;
        }
    }
    Vec::new()
}

/// Port of `kl4a.codekb.relations.resolve_symbol_reference`.
///
/// See the Python docstring/comments (reproduced from evidence): a
/// bundle-wide-unique short name is a safe auto-accept for a *bare*
/// reference (`foo(...)`), but not for a *dotted* one (`x.foo(...)`) since
/// `x`'s type isn't tracked — those fall through to the context-aware
/// checks (`self`/`cls` class-hierarchy lookup, then import-alias lookup),
/// with a shared-prefix-length tiebreak if still ambiguous.
pub fn resolve_symbol_reference(
    name: &str,
    current_symbol: &Value,
    symbols_by_short: &HashMap<String, Vec<Value>>,
    symbol_by_qname: &HashMap<String, Value>,
    module_imports: Option<&[String]>,
    class_bases: Option<&HashMap<String, Vec<String>>>,
) -> Option<Value> {
    let normalized = name.strip_suffix("()").unwrap_or(name);
    let current_module = vstr(current_symbol, "module");
    let current_id = vstr(current_symbol, "id");

    if let Some(direct) = symbol_by_qname
        .get(normalized)
        .or_else(|| symbol_by_qname.get(&format!("{current_module}.{normalized}")))
    {
        return Some(direct.clone());
    }

    let short = normalized.rsplit('.').next().unwrap_or(normalized);
    let candidates: Vec<Value> = symbols_by_short
        .get(short)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| vstr(c, "id") != current_id)
        .collect();

    let is_dotted = normalized.contains('.');
    if !is_dotted {
        if candidates.len() == 1 {
            return Some(candidates[0].clone());
        }
        let same_module: Vec<&Value> = candidates.iter().filter(|c| vstr(c, "module") == current_module).collect();
        if same_module.len() == 1 {
            return Some(same_module[0].clone());
        }
    }

    let mut contextual: Vec<Value> = Vec::new();
    let alias = if is_dotted { normalized.split('.').next() } else { None };

    if matches!(alias, Some("self") | Some("cls")) {
        let current_qname = vstr(current_symbol, "qualified_name");
        if let Some(current_class) = current_qname.rsplit_once('.').map(|(head, _)| head) {
            contextual = members_of_class(&candidates, current_class);
            if contextual.is_empty() {
                if let Some(bases) = class_bases {
                    let mut seen = HashSet::new();
                    contextual = members_of_class_hierarchy(&candidates, current_class, bases, &mut seen);
                }
            }
        }
    }

    if contextual.is_empty() {
        if let Some(imports) = module_imports {
            let lookup_name = alias.unwrap_or(normalized);
            let import_target = imports
                .iter()
                .find(|imp| imp.as_str() == lookup_name || imp.rsplit('.').next() == Some(lookup_name));
            if let Some(import_target) = import_target {
                contextual = candidates
                    .iter()
                    .filter(|c| {
                        let module = vstr(c, "module");
                        let qname = vstr(c, "qualified_name");
                        &module == import_target
                            || module.ends_with(&format!(".{import_target}"))
                            || &qname == import_target
                            || qname.ends_with(&format!(".{import_target}"))
                    })
                    .cloned()
                    .collect();
            }
        }
    }

    if contextual.len() == 1 {
        return Some(contextual[0].clone());
    }
    if contextual.len() > 1 {
        let scored: Vec<(usize, &Value)> = contextual
            .iter()
            .map(|c| (shared_prefix_len(&current_module, &vstr(c, "module")), c))
            .collect();
        let best_score = scored.iter().map(|(s, _)| *s).max().unwrap_or(0);
        let best: Vec<&Value> = scored.into_iter().filter(|(s, _)| *s == best_score).map(|(_, c)| c).collect();
        if best.len() == 1 {
            return Some(best[0].clone());
        }
    }
    None
}

/// Port of `kl4a.codekb.relations.relation_record`.
pub fn relation_record(
    subject: &str,
    predicate: &str,
    object_value: &str,
    subject_kind: &str,
    object_kind: &str,
    resolution_status: &str,
    confidence: f64,
    resolver: &str,
    source_symbol: Option<&Value>,
    source_module: Option<&Value>,
    evidence_ids: &[Option<String>],
) -> Value {
    let relation_id = code_relation_id_for(subject, predicate, object_value);
    let source = source_symbol.or(source_module);
    let language = source.and_then(|s| s.get("language")).and_then(Value::as_str).unwrap_or("python").to_string();
    let file_path = source.and_then(|s| s.get("file")).cloned().unwrap_or(Value::Null);
    let evidence: Vec<Value> = evidence_ids.iter().flatten().map(|s| json!(s)).collect();
    json!({
        "id": relation_id,
        "type": "Code Knowledge Relation",
        "title": format!("{subject} {predicate} {object_value}"),
        "subject": subject,
        "predicate": predicate,
        "object": object_value,
        "subject_kind": subject_kind,
        "object_kind": object_kind,
        "relation": {
            "rdf_compatible": true,
            "resolution_status": resolution_status,
            "resolver": resolver,
            "confidence": (confidence * 100.0).round() / 100.0,
        },
        "code": {"language": language, "file": file_path},
        "evidence": evidence,
        "okf_path": format!("relations/{relation_id}.md"),
    })
}

/// Port of `kl4a.codekb.relations.test_coverage_relations`.
pub fn test_coverage_relations(
    ast_symbol: &SymbolAst,
    symbols_by_short: &HashMap<String, Vec<Value>>,
    symbol_by_qname: &HashMap<String, Value>,
) -> Vec<Value> {
    let mut relations = Vec::new();
    let test_symbol = &ast_symbol.symbol;
    let test_name = vstr(test_symbol, "title");
    let mut candidate_names: HashSet<String> = HashSet::new();
    for prefix in ["test_", "test"] {
        if let Some(rest) = test_name.strip_prefix(prefix) {
            candidate_names.insert(rest.to_string());
        }
    }
    for node in walk_all(&ast_symbol.node) {
        if node.kind() == Kind::Call {
            if let Some(call_name) = node.call_func.as_deref().and_then(call_name_for) {
                if let Some(last) = call_name.rsplit('.').next() {
                    candidate_names.insert(last.to_string());
                }
            }
        }
    }
    let mut names: Vec<&String> = candidate_names.iter().collect();
    names.sort();
    let test_evidence = vstr_opt(test_symbol, "evidence_id");
    for name in names {
        let Some(target) = resolve_symbol_reference(name, test_symbol, symbols_by_short, symbol_by_qname, None, None) else { continue };
        relations.push(relation_record(
            &vid(test_symbol),
            "covers",
            &vid(&target),
            "symbol",
            "symbol",
            "inferred",
            0.7,
            "pytest-name-call-heuristic",
            Some(test_symbol),
            None,
            &[test_evidence.clone()],
        ));
        relations.push(relation_record(
            &vid(&target),
            "tested_by",
            &vid(test_symbol),
            "symbol",
            "symbol",
            "inferred",
            0.7,
            "pytest-name-call-heuristic",
            Some(&target),
            None,
            &[test_evidence.clone()],
        ));
    }
    relations
}

/// `ast.walk`-equivalent (unlike [`pyast::walk_own_scope`], this descends
/// into everything including nested defs) — used only by
/// `test_coverage_relations`, which in Python calls plain `ast.walk`, not
/// `_walk_own_scope`.
fn walk_all(node: &Node) -> Vec<&Node> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        found.push(current);
        stack.extend(current.children.iter());
    }
    found
}

/// Port of `kl4a.codekb.relations.symbol_semantic_relations`.
pub fn symbol_semantic_relations(
    ast_symbol: &SymbolAst,
    symbols_by_short: &HashMap<String, Vec<Value>>,
    symbol_by_qname: &HashMap<String, Value>,
    class_bases: Option<&HashMap<String, Vec<String>>>,
) -> Vec<Value> {
    let symbol = &ast_symbol.symbol;
    let module_imports: Vec<String> = ast_symbol
        .module
        .get("imports")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let mut relations = Vec::new();
    let evidence_id = vstr_opt(symbol, "evidence_id");

    if ast_symbol.node.kind() == Kind::ClassDef {
        for base in &ast_symbol.node.bases {
            let base_name = base.text_or_name();
            let target = resolve_symbol_reference(
                &base_name,
                symbol,
                symbols_by_short,
                symbol_by_qname,
                Some(&module_imports),
                class_bases,
            );
            relations.push(relation_record(
                &vid(symbol),
                "inherits_from",
                &target.as_ref().map(|t| vid(t)).unwrap_or_else(|| base_name.clone()),
                "symbol",
                if target.is_some() { "symbol" } else { "external_symbol" },
                if target.is_some() { "exact" } else { "unresolved" },
                if target.is_some() { 0.96 } else { 0.5 },
                "python-ast",
                Some(symbol),
                None,
                &[evidence_id.clone()],
            ));
        }
    }

    if matches!(ast_symbol.node.kind(), Kind::ClassDef | Kind::FunctionDef | Kind::AsyncFunctionDef) {
        for decorator in &ast_symbol.node.decorator_list {
            let decorator_name = call_name_for(decorator).unwrap_or_else(|| decorator.text_or_name());
            let target = resolve_symbol_reference(
                &decorator_name,
                symbol,
                symbols_by_short,
                symbol_by_qname,
                Some(&module_imports),
                class_bases,
            );
            relations.push(relation_record(
                &vid(symbol),
                "decorated_by",
                &target.as_ref().map(|t| vid(t)).unwrap_or_else(|| decorator_name.clone()),
                "symbol",
                if target.is_some() { "symbol" } else { "decorator" },
                if target.is_some() { "exact" } else { "unresolved" },
                if target.is_some() { 0.95 } else { 0.55 },
                "python-ast",
                Some(symbol),
                None,
                &[evidence_id.clone()],
            ));
        }
    }

    for node in pyast::walk_own_scope(&ast_symbol.node) {
        if node.kind() == Kind::Call {
            if let Some(call_name) = node.call_func.as_deref().and_then(call_name_for) {
                let target = resolve_symbol_reference(
                    &call_name,
                    symbol,
                    symbols_by_short,
                    symbol_by_qname,
                    Some(&module_imports),
                    class_bases,
                );
                relations.push(relation_record(
                    &vid(symbol),
                    "calls",
                    &target.as_ref().map(|t| vid(t)).unwrap_or_else(|| call_name.clone()),
                    "symbol",
                    if target.is_some() { "symbol" } else { "callable" },
                    if target.is_some() { "exact" } else { "unresolved" },
                    if target.is_some() { 0.94 } else { 0.45 },
                    "python-call-resolver",
                    Some(symbol),
                    None,
                    &[evidence_id.clone()],
                ));
            }
            if let Some(env_name) = env_read_name_for(node) {
                relations.push(relation_record(
                    &vid(symbol),
                    "reads",
                    &format!("env:{env_name}"),
                    "symbol",
                    "environment_variable",
                    "exact",
                    0.98,
                    "python-ast",
                    Some(symbol),
                    None,
                    &[evidence_id.clone()],
                ));
            }
            if let Some((predicate, object_value, object_kind)) = file_access_for(node) {
                relations.push(relation_record(
                    &vid(symbol),
                    &predicate,
                    &object_value,
                    "symbol",
                    &object_kind,
                    "inferred",
                    0.72,
                    "python-file-access-heuristic",
                    Some(symbol),
                    None,
                    &[evidence_id.clone()],
                ));
            }
        } else if node.kind() == Kind::Raise {
            if let Some(exception_name) = exception_name_for(node) {
                relations.push(relation_record(
                    &vid(symbol),
                    "raises",
                    &exception_name,
                    "symbol",
                    "exception",
                    "exact",
                    0.96,
                    "python-ast",
                    Some(symbol),
                    None,
                    &[evidence_id.clone()],
                ));
            }
        }
    }

    if symbol.get("kind").and_then(Value::as_str) == Some("test") {
        relations.extend(test_coverage_relations(ast_symbol, symbols_by_short, symbol_by_qname));
    }

    relations
}

/// Port of `kl4a.codekb.relations.dedupe_relations`.
pub fn dedupe_relations(relations: Vec<Value>) -> Vec<Value> {
    let mut by_key: HashMap<(String, String, String), Value> = HashMap::new();
    let mut order: Vec<(String, String, String)> = Vec::new();
    for relation in relations {
        let key = (vstr(&relation, "subject"), vstr(&relation, "predicate"), vstr(&relation, "object"));
        match by_key.get_mut(&key) {
            None => {
                order.push(key.clone());
                by_key.insert(key, relation);
            }
            Some(existing) => {
                let mut merged_evidence: HashSet<String> = existing
                    .get("evidence")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                merged_evidence.extend(
                    relation
                        .get("evidence")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>())
                        .unwrap_or_default(),
                );
                let mut sorted_evidence: Vec<String> = merged_evidence.into_iter().collect();
                sorted_evidence.sort();
                if let Some(obj) = existing.as_object_mut() {
                    obj.insert("evidence".into(), json!(sorted_evidence));
                }
                let existing_status = existing.get("relation").and_then(|r| r.get("resolution_status")).and_then(Value::as_str);
                let new_status = relation.get("relation").and_then(|r| r.get("resolution_status")).and_then(Value::as_str);
                if existing_status != Some("exact") && new_status == Some("exact") {
                    let evidence = existing.get("evidence").cloned().unwrap_or(json!([]));
                    let mut replacement = relation;
                    if let Some(obj) = replacement.as_object_mut() {
                        obj.insert("evidence".into(), evidence);
                    }
                    *existing = replacement;
                }
            }
        }
    }
    let mut result: Vec<Value> = order.into_iter().map(|k| by_key.remove(&k).unwrap()).collect();
    result.sort_by(|a, b| {
        let ka = (vstr(a, "predicate"), vstr(a, "subject"), vstr(a, "object"));
        let kb = (vstr(b, "predicate"), vstr(b, "subject"), vstr(b, "object"));
        ka.cmp(&kb)
    });
    result
}

/// Port of `kl4a.codekb.relations.relation_summary`.
pub fn relation_summary(relations: &[Value]) -> Value {
    let mut by_predicate: Map<String, Value> = Map::new();
    let mut by_status: Map<String, Value> = Map::new();
    for relation in relations {
        let predicate = vstr(relation, "predicate");
        let count = by_predicate.get(&predicate).and_then(Value::as_i64).unwrap_or(0);
        by_predicate.insert(predicate, json!(count + 1));
        let status = relation.get("relation").and_then(|r| r.get("resolution_status")).and_then(Value::as_str).unwrap_or("").to_string();
        let scount = by_status.get(&status).and_then(Value::as_i64).unwrap_or(0);
        by_status.insert(status, json!(scount + 1));
    }
    json!({
        "count": relations.len(),
        "by_predicate": by_predicate,
        "by_resolution_status": by_status,
    })
}

/// Port of `kl4a.codekb.relations.write_relation_doc`.
pub fn write_relation_doc(bundle_dir: &Path, relation: &Value) -> Result<()> {
    let okf_path = vstr(relation, "okf_path");
    let mut frontmatter = Map::new();
    frontmatter.insert("type".into(), relation.get("type").cloned().unwrap_or(Value::Null));
    frontmatter.insert("title".into(), relation.get("title").cloned().unwrap_or(Value::Null));
    frontmatter.insert("relation_id".into(), relation.get("id").cloned().unwrap_or(Value::Null));
    frontmatter.insert("subject".into(), relation.get("subject").cloned().unwrap_or(Value::Null));
    frontmatter.insert("predicate".into(), relation.get("predicate").cloned().unwrap_or(Value::Null));
    frontmatter.insert("object".into(), relation.get("object").cloned().unwrap_or(Value::Null));
    frontmatter.insert("subject_kind".into(), relation.get("subject_kind").cloned().unwrap_or(Value::Null));
    frontmatter.insert("object_kind".into(), relation.get("object_kind").cloned().unwrap_or(Value::Null));
    frontmatter.insert("relation".into(), relation.get("relation").cloned().unwrap_or(Value::Null));
    frontmatter.insert("code".into(), relation.get("code").cloned().unwrap_or(Value::Null));
    frontmatter.insert("evidence".into(), relation.get("evidence").cloned().unwrap_or(Value::Null));
    let body = format!(
        "# {}\n\n`{}` `{}` `{}`\n",
        vstr(relation, "title"),
        vstr(relation, "subject"),
        vstr(relation, "predicate"),
        vstr(relation, "object"),
    );
    write_markdown(&bundle_dir.join(okf_path), &Value::Object(frontmatter), &body)
}

/// Port of `kl4a.codekb.relations.generate_code_relations`.
///
/// Every branch of the original is ported: `defines`/`imports` relations
/// from the parsed module/symbol state, per-symbol semantic relations via
/// [`symbol_semantic_relations`] (Python-only), a pass-through to the
/// COBOL adapter's own relation generator for non-Python relations, then
/// dedup + per-relation doc write + summary + state write.
pub fn generate_code_relations(bundle_dir: &Path) -> Result<Value> {
    let inventory = read_json(&bundle_dir.join(STATE_DIR).join("code_inventory.json"), json!({"sources": []}));
    let symbols_state = read_json(
        &bundle_dir.join(STATE_DIR).join("code_symbols.json"),
        json!({"modules": [], "symbols": [], "evidence": []}),
    );

    let sources_by_id: HashMap<String, Value> = inventory
        .get("sources")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|s| (vid(&s), s))
        .collect();
    let modules: Vec<Value> = symbols_state.get("modules").and_then(Value::as_array).cloned().unwrap_or_default();
    let modules_by_id: HashMap<String, Value> = modules.iter().map(|m| (vid(m), m.clone())).collect();
    let symbols: Vec<Value> = symbols_state.get("symbols").and_then(Value::as_array).cloned().unwrap_or_default();
    let symbol_by_id: HashMap<String, Value> = symbols.iter().map(|s| (vid(s), s.clone())).collect();
    let symbol_by_qname: HashMap<String, Value> = symbols.iter().map(|s| (vstr(s, "qualified_name"), s.clone())).collect();
    let symbols_by_short = build_short_symbol_index(&symbols);
    let ast_symbols = load_symbol_asts(bundle_dir, &sources_by_id, &modules_by_id, &symbol_by_id);
    let class_bases = build_class_bases(&ast_symbols);

    let mut relations: Vec<Value> = Vec::new();
    for module in &modules {
        let module_id = vid(module);
        let module_symbols: Vec<&Value> = symbols.iter().filter(|s| vstr_opt(s, "module_id").as_deref() == Some(module_id.as_str())).collect();
        for symbol in &module_symbols {
            relations.push(relation_record(
                &module_id,
                "defines",
                &vid(symbol),
                "module",
                "symbol",
                "exact",
                1.0,
                "python-ast",
                Some(symbol),
                None,
                &[vstr_opt(symbol, "evidence_id")],
            ));
        }
        let imports = module.get("imports").and_then(Value::as_array).cloned().unwrap_or_default();
        for import_name in imports {
            let Some(import_name) = import_name.as_str() else { continue };
            let target = resolve_import_target(import_name, &modules, &vstr(module, "qualified_name"));
            relations.push(relation_record(
                &module_id,
                "imports",
                &target.as_ref().map(vid).unwrap_or_else(|| import_name.to_string()),
                "module",
                if target.is_some() { "module" } else { "external_module" },
                if target.is_some() { "exact" } else { "unresolved" },
                if target.is_some() { 0.96 } else { 0.45 },
                "python-import-resolver",
                None,
                Some(module),
                &[],
            ));
        }
    }

    for ast_symbol in &ast_symbols {
        relations.extend(symbol_semantic_relations(ast_symbol, &symbols_by_short, &symbol_by_qname, Some(&class_bases)));
    }
    relations.extend(crate::adapters::cobol_adapter::generate_cobol_relations(bundle_dir, &symbols_state)?);

    let relations = dedupe_relations(relations);
    for relation in &relations {
        write_relation_doc(bundle_dir, relation)?;
    }

    let result = json!({
        "relations": relations,
        "summary": relation_summary(&relations),
    });
    write_code_state(bundle_dir, "code_relations.json", &result)?;
    Ok(result)
}
