//! Port of `kl4a/codekb/render.py`.
//!
//! Composes the bundle's human-facing Markdown layer (symbol/module bodies,
//! indexes, the architecture overview, request-flow docs) from the persisted
//! JSON state that `CodeBundle` (`crate::model`) already loads. Every symbol
//! in the Python file (the module-level `render_human_layer` plus every
//! method of `CodeRenderer`, public and private, including the three nested
//! closures inside `_mermaid_layered_flow`) is ported below — see the
//! completeness ledger in the batch report for the source-file mapping.
//!
//! # WIRING (external dependencies not in this batch)
//!
//! `render.py` leans on four modules this batch was not scoped to port:
//! - `kl4a.kl4a.bundle_store.load_manifest` — same gap `model.rs` already
//!   flags (`crate::bundle_store::load_manifest`).
//! - `kl4a.codekb.relations.resolve_import_target` — assumed here as
//!   `crate::relations::resolve_import_target(import_name: &str, modules:
//!   &[Value]) -> Option<Value>` (returns a cloned module doc, matching the
//!   Python "returns a module doc or None" signature). UNCONFIRMED shape.
//! - `kl4a.codekb.bundle.rel_link` — assumed here as
//!   `crate::bundle::rel_link(from_path: &str, target_path: &str) ->
//!   String`. UNCONFIRMED shape.
//! - `kl4a.kl4a.okf_writer.write_markdown` — grounded via MCP:
//!   `write_markdown(path: Path, frontmatter: dict[str, Any], body: str) ->
//!   None`, so `crate::okf_writer::write_markdown(path: &Path, frontmatter:
//!   &Value, body: &str) -> Result<()>` follows that exactly.
//! - `OKFDocument.parse` (used in `_rewrite_body` to recover an existing
//!   file's frontmatter) — the tools-code MCP graph could not resolve this
//!   call to a specific module (an "unresolved callable", not an exact
//!   symbol), so its home module is genuinely UNCONFIRMED. Guessed here as
//!   `crate::okf_writer::OkfDocument` (colocated with `write_markdown`) with
//!   a `parse(text: &str) -> Result<OkfDocument>` and a `frontmatter: Option<
//!   Value>` field — the coordinator must verify/relocate this against
//!   whichever module actually defines it.
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{Map, Value};

use crate::model::{get_code_bundle, CodeBundle};

// See the module-level WIRING note above for all four of these.
use crate::bundle::rel_link;
use crate::bundle_store::load_manifest;
use crate::okf_writer::{write_markdown, OkfDocument};
use crate::relations::resolve_import_target;

/// Mirrors `kl4a.codekb.render._KIND_ORDER`.
const KIND_ORDER: &[&str] = &["class", "method", "function", "test", "constant"];

/// Mirrors `kl4a.codekb.render._KIND_LABEL`.
static KIND_LABEL: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    [
        ("class", "Classes"),
        ("method", "Methods"),
        ("function", "Functions"),
        ("test", "Tests"),
        ("constant", "Constants"),
    ]
    .into_iter()
    .collect()
});

/// Mirrors `kl4a.codekb.render._MAX_SIG`.
const MAX_SIG: usize = 200;

static WHITESPACE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

/// Mirrors `kl4a.codekb.render.render_human_layer`: "Compose the
/// human-facing docs from persisted state. Returns a summary."
pub fn render_human_layer(bundle_dir: &Path) -> Result<Value> {
    let renderer = CodeRenderer::new(bundle_dir)?;
    renderer.render()
}

/// Mirrors `kl4a.codekb.render.CodeRenderer`.
pub struct CodeRenderer {
    bundle_dir: PathBuf,
    cb: Arc<CodeBundle>,
    manifest: Value,
    /// symbol index (into `cb.symbols`), grouped by `symbol["module_id"]`.
    symbols_by_module: HashMap<String, Vec<usize>>,
    /// module doc, by `module["qualified_name"]`.
    modules_by_qname: HashMap<String, Value>,
    /// reverse import graph: module id -> importing module ids.
    imported_by: HashMap<String, HashSet<String>>,
    /// module id -> `(import_name, resolved_target_module_doc)` pairs.
    module_imports: HashMap<String, Vec<(String, Option<Value>)>>,
    /// symbol ids referenced by `cb.data_models[*]["symbol_id"]`.
    model_symbol_ids: HashSet<String>,
    /// symbol index (into `cb.symbols`), by `symbol["qualified_name"]`. Built
    /// for parity with the Python constructor; no method in this file reads
    /// it back (same as the Python source, where it is likewise write-only
    /// within the symbols captured by this port).
    #[allow(dead_code)]
    qname_to_symbol: HashMap<String, usize>,
}

impl CodeRenderer {
    /// Mirrors `CodeRenderer.__init__`.
    pub fn new(bundle_dir: &Path) -> Result<Self> {
        let cb = get_code_bundle(bundle_dir)?;
        let manifest = load_manifest(bundle_dir).unwrap_or_else(|_| Value::Object(Map::new()));

        let mut symbols_by_module: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, symbol) in cb.symbols.iter().enumerate() {
            let module_id = symbol
                .get("module_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            symbols_by_module.entry(module_id).or_default().push(idx);
        }

        let mut modules_by_qname: HashMap<String, Value> = HashMap::new();
        for m in &cb.modules {
            if let Some(q) = m.get("qualified_name").and_then(Value::as_str) {
                modules_by_qname.insert(q.to_string(), m.clone());
            }
        }

        let mut imported_by: HashMap<String, HashSet<String>> = HashMap::new();
        let mut module_imports: HashMap<String, Vec<(String, Option<Value>)>> = HashMap::new();
        for module in &cb.modules {
            let module_id = module.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            let mut resolved: Vec<(String, Option<Value>)> = Vec::new();
            if let Some(imports) = module.get("imports").and_then(Value::as_array) {
                for import_value in imports {
                    let import_name = import_value.as_str().unwrap_or("").to_string();
                    let importing_module = module.get("qualified_name").and_then(Value::as_str).unwrap_or("");
                    let target = resolve_import_for(&cb, &modules_by_qname, &import_name, importing_module);
                    if let Some(target) = &target {
                        let target_id = target.get("id").and_then(Value::as_str).unwrap_or("");
                        if target_id != module_id {
                            imported_by
                                .entry(target_id.to_string())
                                .or_default()
                                .insert(module_id.clone());
                        }
                    }
                    resolved.push((import_name, target));
                }
            }
            module_imports.insert(module_id, resolved);
        }

        let model_symbol_ids: HashSet<String> = cb
            .data_models
            .iter()
            .filter_map(|m| m.get("symbol_id").and_then(Value::as_str))
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();

        let mut qname_to_symbol: HashMap<String, usize> = HashMap::new();
        for (idx, s) in cb.symbols.iter().enumerate() {
            if let Some(q) = s.get("qualified_name").and_then(Value::as_str) {
                qname_to_symbol.insert(q.to_string(), idx);
            }
        }

        Ok(CodeRenderer {
            bundle_dir: bundle_dir.to_path_buf(),
            cb,
            manifest,
            symbols_by_module,
            modules_by_qname,
            imported_by,
            module_imports,
            model_symbol_ids,
            qname_to_symbol,
        })
    }

    fn symbol(&self, id: &str) -> Option<&Value> {
        self.cb.symbol_by_id.get(id).map(|&i| &self.cb.symbols[i])
    }

    fn module(&self, id: &str) -> Option<&Value> {
        self.cb.module_by_id.get(id).map(|&i| &self.cb.modules[i])
    }

    fn symbols_for_module(&self, module_id: &str) -> Vec<usize> {
        self.symbols_by_module.get(module_id).cloned().unwrap_or_default()
    }

    /// Mirrors `CodeRenderer._resolve_import`. Kept as an instance method for
    /// fidelity with the Python class surface, even though (matching the
    /// Python source) the only call site is inside the constructor, via the
    /// free function `resolve_import_for` below (needed there because `self`
    /// does not exist yet while building `modules_by_qname`).
    #[allow(dead_code)]
    fn resolve_import(&self, import_name: &str) -> Option<Value> {
        // Dead code (see doc comment above) -- no specific importing module
        // in scope here, matching the Python method's own class-level call
        // shape; the real, live call site is `resolve_import_for` below,
        // which always has a concrete importing module's qualified name.
        resolve_import_for(&self.cb, &self.modules_by_qname, import_name, "")
    }

    /// Mirrors `CodeRenderer._link`.
    fn link(&self, from_path: &str, target_okf_path: &str, label: &str) -> String {
        format!("[{label}]({})", rel_link(from_path, target_okf_path))
    }

    /// Mirrors `CodeRenderer._symbol_link`.
    fn symbol_link(&self, from_path: &str, symbol_id: &str) -> String {
        match self.symbol(symbol_id) {
            Some(sym) => self.link(
                from_path,
                sym.get("okf_path").and_then(Value::as_str).unwrap_or(""),
                sym.get("qualified_name").and_then(Value::as_str).unwrap_or(""),
            ),
            None => format!("`{symbol_id}`"),
        }
    }

    /// Mirrors `CodeRenderer._object_render`.
    fn object_render(&self, from_path: &str, obj: &str) -> String {
        if let Some(sym) = self.symbol(obj) {
            return self.link(
                from_path,
                sym.get("okf_path").and_then(Value::as_str).unwrap_or(""),
                sym.get("qualified_name").and_then(Value::as_str).unwrap_or(""),
            );
        }
        if let Some(module) = self.module(obj) {
            return self.link(
                from_path,
                module.get("okf_path").and_then(Value::as_str).unwrap_or(""),
                module.get("qualified_name").and_then(Value::as_str).unwrap_or(""),
            );
        }
        let label = obj.strip_prefix("env:").unwrap_or(obj);
        format!("`{label}`")
    }

    /// Mirrors `CodeRenderer._rewrite_body`.
    fn rewrite_body(&self, okf_path: &str, body: &str, default_frontmatter: Value) -> Result<()> {
        let path = self.bundle_dir.join(okf_path);
        let frontmatter = if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            let parsed = OkfDocument::parse(&text)?;
            if parsed.frontmatter.is_empty() {
                default_frontmatter
            } else {
                Value::Object(parsed.frontmatter)
            }
        } else {
            default_frontmatter
        };
        write_markdown(&path, &frontmatter, body)
    }

    /// Mirrors `CodeRenderer._relation_section`.
    fn relation_section(
        &self,
        heading: &str,
        from_path: &str,
        relations: &[Value],
        predicate: &str,
        use_object: bool,
    ) -> Vec<String> {
        let field = if use_object { "object" } else { "subject" };
        let mut picked: Vec<&Value> = relations
            .iter()
            .filter(|r| r.get("predicate").and_then(Value::as_str) == Some(predicate))
            .collect();
        if picked.is_empty() {
            return Vec::new();
        }
        picked.sort_by_key(|r| r.get(field).map(value_to_key_string).unwrap_or_default());

        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<String> = Vec::new();
        for rel in picked {
            let target = rel.get(field).map(value_to_key_string).unwrap_or_default();
            if seen.contains(&target) {
                continue;
            }
            seen.insert(target.clone());
            // Literal port of `(rel.get("relation") or {}).get("resolution_status", "")`:
            // a plain relation record (as stored in `cb.relations`) carries
            // `resolution_status` at the top level, not nested under a
            // `"relation"` key, so this is effectively always "" for real
            // data — kept as-is (the mechanism, not a guess at what it
            // "should" do) per the grounded source text.
            let status = rel
                .get("relation")
                .and_then(Value::as_object)
                .and_then(|o| o.get("resolution_status"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let suffix = if status == "exact" { String::new() } else { format!(" _({status})_") };
            items.push(format!("- {}{suffix}", self.object_render(from_path, &target)));
        }
        let mut out = vec![format!("## {heading}"), String::new()];
        out.extend(items);
        out.push(String::new());
        out
    }

    /// Mirrors `CodeRenderer._io_section`.
    fn io_section(&self, from_path: &str, out_rel: &[Value]) -> Vec<String> {
        let reads: Vec<&Value> = out_rel
            .iter()
            .filter(|r| r.get("predicate").and_then(Value::as_str) == Some("reads"))
            .collect();
        let writes: Vec<&Value> = out_rel
            .iter()
            .filter(|r| r.get("predicate").and_then(Value::as_str) == Some("writes"))
            .collect();
        if reads.is_empty() && writes.is_empty() {
            return Vec::new();
        }
        let mut lines = vec!["## Reads / Writes".to_string(), String::new()];
        for (label, group) in [("reads", &reads), ("writes", &writes)] {
            let mut targets: BTreeSet<String> = BTreeSet::new();
            for r in group.iter() {
                targets.insert(r.get("object").map(value_to_key_string).unwrap_or_default());
            }
            for target in targets {
                lines.push(format!("- {label}: {}", self.object_render(from_path, &target)));
            }
        }
        lines.push(String::new());
        lines
    }

    /// Mirrors `CodeRenderer._primary_claim`.
    fn primary_claim(&self, symbol_id: &str) -> Option<String> {
        let item_idxs = self.cb.items_by_symbol.get(symbol_id)?;
        if item_idxs.is_empty() {
            return None;
        }
        let best_idx = *item_idxs.iter().min_by_key(|&&idx| {
            self.cb.items[idx].get("knowledge_tier").and_then(Value::as_i64).unwrap_or(99)
        })?;
        let best = &self.cb.items[best_idx];
        let claim = best.get("claim").and_then(Value::as_str).filter(|s| !s.is_empty());
        let title = best.get("title").and_then(Value::as_str).filter(|s| !s.is_empty());
        claim.or(title).map(str::to_string)
    }

    /// Mirrors `CodeRenderer.render_symbol_bodies`.
    pub fn render_symbol_bodies(&self) -> Result<()> {
        for symbol in &self.cb.symbols {
            let okf_path = symbol.get("okf_path").and_then(Value::as_str).unwrap_or("").to_string();
            let body = self.symbol_body(symbol);
            self.rewrite_body(&okf_path, &body, symbol_frontmatter(symbol))?;
        }
        Ok(())
    }

    /// Mirrors `CodeRenderer._symbol_body`.
    fn symbol_body(&self, symbol: &Value) -> String {
        let okf_path = symbol.get("okf_path").and_then(Value::as_str).unwrap_or("").to_string();
        let sid = symbol.get("id").and_then(Value::as_str).unwrap_or("").to_string();
        let mut lines: Vec<String> =
            vec![format!("# {}", symbol.get("title").and_then(Value::as_str).unwrap_or("")), String::new()];

        let module = symbol.get("module_id").and_then(Value::as_str).and_then(|id| self.module(id));
        let mut meta = vec![
            format!("`{}`", symbol.get("qualified_name").and_then(Value::as_str).unwrap_or("")),
            format!("_{}_", symbol.get("kind").and_then(Value::as_str).unwrap_or("")),
        ];
        if let Some(module) = module {
            meta.push(format!(
                "in {}",
                self.link(
                    &okf_path,
                    module.get("okf_path").and_then(Value::as_str).unwrap_or(""),
                    module.get("qualified_name").and_then(Value::as_str).unwrap_or("")
                )
            ));
        }
        lines.push(meta.join(" \u{b7} "));
        lines.push(String::new());

        if let Some(sig) = short_signature(symbol.get("signature").and_then(Value::as_str)) {
            lines.push("```python".to_string());
            lines.push(sig);
            lines.push("```".to_string());
            lines.push(String::new());
        }

        let blurb = symbol
            .get("docstring")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| self.primary_claim(&sid));
        if let Some(blurb) = blurb {
            lines.push(first_paragraph(&blurb));
            lines.push(String::new());
        }

        let out_rel: Vec<Value> = self
            .cb
            .out_rel
            .get(&sid)
            .map(|idxs| idxs.iter().map(|&i| self.cb.relations[i].clone()).collect())
            .unwrap_or_default();
        let in_rel: Vec<Value> = self
            .cb
            .in_rel
            .get(&sid)
            .map(|idxs| idxs.iter().map(|&i| self.cb.relations[i].clone()).collect())
            .unwrap_or_default();

        lines.extend(self.relation_section("Calls", &okf_path, &out_rel, "calls", true));
        lines.extend(self.relation_section("Called by", &okf_path, &in_rel, "calls", false));
        lines.extend(self.relation_section("Inherits from", &okf_path, &out_rel, "inherits_from", true));
        lines.extend(self.relation_section("Decorated by", &okf_path, &out_rel, "decorated_by", true));
        lines.extend(self.relation_section("Raises", &okf_path, &out_rel, "raises", true));
        lines.extend(self.io_section(&okf_path, &out_rel));
        lines.extend(self.relation_section("Tested by", &okf_path, &out_rel, "tested_by", true));

        let knowledge: Vec<usize> = self.cb.items_by_symbol.get(&sid).cloned().unwrap_or_default();
        if !knowledge.is_empty() {
            lines.push("## Knowledge".to_string());
            lines.push(String::new());
            let mut sorted = knowledge;
            sorted.sort_by_key(|&idx| {
                self.cb.items[idx].get("knowledge_tier").and_then(Value::as_i64).unwrap_or(0)
            });
            for idx in sorted {
                let item = &self.cb.items[idx];
                let tier = value_display_opt(item.get("knowledge_tier"));
                let status = item.get("review_status").and_then(Value::as_str).unwrap_or("");
                let title = item
                    .get("title")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("id").and_then(Value::as_str))
                    .unwrap_or("");
                let link = self.link(&okf_path, item.get("okf_path").and_then(Value::as_str).unwrap_or(""), title);
                lines.push(format!("- {link} \u{2014} Tier {tier} ({status})"));
            }
            lines.push(String::new());
        }

        if let Some(evidence_id) = symbol.get("evidence_id").and_then(Value::as_str).filter(|s| !s.is_empty()) {
            let ev_path = format!("evidence/{evidence_id}.md");
            let span = format!(
                "lines {}-{}",
                value_display_opt(symbol.get("line_start")),
                value_display_opt(symbol.get("line_end"))
            );
            lines.push("## Evidence".to_string());
            lines.push(String::new());
            lines.push(format!(
                "`{}` {}",
                symbol.get("file").and_then(Value::as_str).unwrap_or(""),
                self.link(&okf_path, &ev_path, &span)
            ));
            lines.push(String::new());
        }

        format!("{}\n", lines.join("\n").trim_end())
    }

    /// Mirrors `CodeRenderer.render_module_bodies`.
    pub fn render_module_bodies(&self) -> Result<()> {
        for module in &self.cb.modules {
            let okf_path = module.get("okf_path").and_then(Value::as_str).unwrap_or("").to_string();
            let body = self.module_body(module);
            self.rewrite_body(&okf_path, &body, module_frontmatter(module))?;
        }
        Ok(())
    }

    /// Mirrors `CodeRenderer._module_body`.
    fn module_body(&self, module: &Value) -> String {
        let okf_path = module.get("okf_path").and_then(Value::as_str).unwrap_or("").to_string();
        let qname = module.get("qualified_name").and_then(Value::as_str).unwrap_or("");
        let mut lines = vec![format!("# {qname}"), String::new()];

        let source_id = module.get("source_id").and_then(Value::as_str).unwrap_or("");
        let file_label = module.get("file").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("source");
        lines.push(format!(
            "Source: {}",
            self.link(&okf_path, &format!("sources/files/{source_id}.md"), file_label)
        ));
        lines.push(String::new());

        let module_id = module.get("id").and_then(Value::as_str).unwrap_or("");
        let symbols = self.symbols_for_module(module_id);
        if !symbols.is_empty() {
            lines.push(format!("## Defined symbols ({})", symbols.len()));
            lines.push(String::new());
            let mut by_kind: HashMap<&str, Vec<usize>> = HashMap::new();
            for &idx in &symbols {
                let kind = self.cb.symbols[idx].get("kind").and_then(Value::as_str).unwrap_or("other");
                by_kind.entry(kind).or_default().push(idx);
            }
            let mut ordered_kinds: Vec<&str> = KIND_ORDER.to_vec();
            let mut extra: Vec<&str> = by_kind.keys().copied().filter(|k| !KIND_ORDER.contains(k)).collect();
            extra.sort();
            ordered_kinds.extend(extra);
            for kind in ordered_kinds {
                let Some(group) = by_kind.get(kind) else { continue };
                if group.is_empty() {
                    continue;
                }
                let label = KIND_LABEL.get(kind).map(|s| s.to_string()).unwrap_or_else(|| title_case(kind));
                lines.push(format!("**{label}**"));
                lines.push(String::new());
                let mut group_sorted = group.clone();
                group_sorted.sort_by_key(|&idx| {
                    self.cb.symbols[idx].get("line_start").and_then(Value::as_i64).unwrap_or(0)
                });
                for idx in group_sorted {
                    let sym = &self.cb.symbols[idx];
                    lines.push(format!(
                        "- {}",
                        self.link(
                            &okf_path,
                            sym.get("okf_path").and_then(Value::as_str).unwrap_or(""),
                            sym.get("qualified_name").and_then(Value::as_str).unwrap_or("")
                        )
                    ));
                }
                lines.push(String::new());
            }
        }

        let resolved = self.module_imports.get(module_id).cloned().unwrap_or_default();
        if !resolved.is_empty() {
            lines.push("## Imports".to_string());
            lines.push(String::new());
            for (import_name, target) in &resolved {
                match target {
                    Some(target) => lines.push(format!(
                        "- {}",
                        self.link(
                            &okf_path,
                            target.get("okf_path").and_then(Value::as_str).unwrap_or(""),
                            target.get("qualified_name").and_then(Value::as_str).unwrap_or("")
                        )
                    )),
                    None => lines.push(format!("- `{import_name}`")),
                }
            }
            lines.push(String::new());
        }

        let mut importers: Vec<String> =
            self.imported_by.get(module_id).cloned().unwrap_or_default().into_iter().collect();
        importers.sort();
        if !importers.is_empty() {
            lines.push("## Imported by".to_string());
            lines.push(String::new());
            for mid in importers {
                if let Some(m) = self.module(&mid) {
                    lines.push(format!(
                        "- {}",
                        self.link(
                            &okf_path,
                            m.get("okf_path").and_then(Value::as_str).unwrap_or(""),
                            m.get("qualified_name").and_then(Value::as_str).unwrap_or("")
                        )
                    ));
                }
            }
            lines.push(String::new());
        }

        let endpoints: Vec<&Value> =
            self.cb.endpoints.iter().filter(|e| e.get("module").and_then(Value::as_str) == Some(qname)).collect();
        if !endpoints.is_empty() {
            lines.push("## Endpoints".to_string());
            lines.push(String::new());
            for ep in endpoints {
                lines.push(format!(
                    "- `{} {}`",
                    ep.get("method").and_then(Value::as_str).unwrap_or(""),
                    ep.get("path").and_then(Value::as_str).unwrap_or("")
                ));
            }
            lines.push(String::new());
        }

        format!("{}\n", lines.join("\n").trim_end())
    }

    /// Mirrors `CodeRenderer.render_indexes`.
    pub fn render_indexes(&self) -> Result<()> {
        self.render_modules_index()?;
        self.render_symbols_index()?;
        self.render_sources_index()?;
        self.render_code_index()?;
        self.render_arch_indexes()?;
        self.render_root_index()?;
        Ok(())
    }

    /// Mirrors `CodeRenderer._render_modules_index`.
    fn render_modules_index(&self) -> Result<()> {
        let okf_path = "code/modules/index.md";
        let mut by_package: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, module) in self.cb.modules.iter().enumerate() {
            let qname = module.get("qualified_name").and_then(Value::as_str).unwrap_or("");
            let pkg = qname.split('.').next().unwrap_or("").to_string();
            by_package.entry(pkg).or_default().push(idx);
        }
        let mut lines = vec![format!("# Modules ({})", self.cb.modules.len()), String::new()];
        let mut packages: Vec<&String> = by_package.keys().collect();
        packages.sort();
        for pkg in packages {
            lines.push(format!("## {pkg}"));
            lines.push(String::new());
            let mut idxs = by_package[pkg].clone();
            idxs.sort_by_key(|&i| {
                self.cb.modules[i].get("qualified_name").and_then(Value::as_str).unwrap_or("").to_string()
            });
            for idx in idxs {
                let module = &self.cb.modules[idx];
                let module_id = module.get("id").and_then(Value::as_str).unwrap_or("");
                let count = self.symbols_for_module(module_id).len();
                let link = self.link(
                    okf_path,
                    module.get("okf_path").and_then(Value::as_str).unwrap_or(""),
                    module.get("qualified_name").and_then(Value::as_str).unwrap_or(""),
                );
                lines.push(format!("- {link} \u{2014} {count} symbol(s)"));
            }
            lines.push(String::new());
        }
        self.rewrite_body(okf_path, &format!("{}\n", lines.join("\n").trim_end()), index_frontmatter("Modules"))
    }

    /// Mirrors `CodeRenderer._render_symbols_index`.
    fn render_symbols_index(&self) -> Result<()> {
        let okf_path = "code/symbols/index.md";
        let mut counts: HashMap<String, usize> = HashMap::new();
        for sym in &self.cb.symbols {
            let kind = sym.get("kind").and_then(Value::as_str).unwrap_or("other").to_string();
            *counts.entry(kind).or_insert(0) += 1;
        }
        let mut lines = vec![format!("# Symbols ({})", self.cb.symbols.len()), String::new()];
        let mut keys: Vec<&String> = counts.keys().collect();
        keys.sort();
        let summary = if keys.is_empty() {
            "none".to_string()
        } else {
            keys.iter().map(|k| format!("{k}: {}", counts[*k])).collect::<Vec<_>>().join(" \u{b7} ")
        };
        lines.push(summary);
        lines.push(String::new());
        lines.push("Symbols are grouped by module.".to_string());
        lines.push(String::new());

        let mut modules_sorted: Vec<&Value> = self.cb.modules.iter().collect();
        modules_sorted
            .sort_by_key(|m| m.get("qualified_name").and_then(Value::as_str).unwrap_or("").to_string());
        for module in modules_sorted {
            let module_id = module.get("id").and_then(Value::as_str).unwrap_or("");
            let mut symbols = self.symbols_for_module(module_id);
            if symbols.is_empty() {
                continue;
            }
            lines.push(format!("## {}", module.get("qualified_name").and_then(Value::as_str).unwrap_or("")));
            lines.push(String::new());
            symbols.sort_by_key(|&i| self.cb.symbols[i].get("line_start").and_then(Value::as_i64).unwrap_or(0));
            for idx in symbols {
                let sym = &self.cb.symbols[idx];
                let link = self.link(
                    okf_path,
                    sym.get("okf_path").and_then(Value::as_str).unwrap_or(""),
                    sym.get("title").and_then(Value::as_str).unwrap_or(""),
                );
                lines.push(format!(
                    "- {link} \u{2014} _{}_ `{}`",
                    sym.get("kind").and_then(Value::as_str).unwrap_or(""),
                    sym.get("qualified_name").and_then(Value::as_str).unwrap_or("")
                ));
            }
            lines.push(String::new());
        }
        self.rewrite_body(okf_path, &format!("{}\n", lines.join("\n").trim_end()), index_frontmatter("Symbols"))
    }

    /// Mirrors `CodeRenderer._render_sources_index`.
    fn render_sources_index(&self) -> Result<()> {
        for (okf_path, title) in [("sources/index.md", "Code Sources"), ("sources/files/index.md", "Source Files")] {
            let mut lines = vec![format!("# {title} ({})", self.cb.sources.len()), String::new()];
            let mut sources: Vec<&Value> = self.cb.sources.iter().collect();
            sources.sort_by_key(|s| s.get("path").and_then(Value::as_str).unwrap_or("").to_string());
            for src in sources {
                let path = src.get("path").and_then(Value::as_str).unwrap_or("");
                let id = src.get("id").and_then(Value::as_str).unwrap_or("");
                let target = src
                    .get("okf_path")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("sources/files/{id}.md"));
                let count = self.cb.symbols_by_source.get(path).map(|v| v.len()).unwrap_or(0);
                let lang = src.get("language").and_then(Value::as_str).unwrap_or("");
                let label = if path.is_empty() { id } else { path };
                lines.push(format!("- {} \u{2014} {lang}, {count} symbol(s)", self.link(okf_path, &target, label)));
            }
            self.rewrite_body(okf_path, &format!("{}\n", lines.join("\n").trim_end()), index_frontmatter(title))?;
        }
        Ok(())
    }

    /// Mirrors `CodeRenderer._render_code_index`.
    fn render_code_index(&self) -> Result<()> {
        let okf_path = "code/index.md";
        let rows: [(&str, &str, usize); 6] = [
            ("Modules", "modules/index.md", self.cb.modules.len()),
            ("Symbols", "symbols/index.md", self.cb.symbols.len()),
            ("Endpoints", "endpoints/index.md", self.cb.endpoints.len()),
            ("Data models", "data-models/index.md", self.cb.data_models.len()),
            ("Schemas", "schemas/index.md", self.cb.schemas.len()),
            ("Dependencies", "dependencies/index.md", self.cb.dependencies.len()),
        ];
        let mut lines = vec!["# Code".to_string(), String::new()];
        for (label, rel, count) in rows {
            if count > 0 {
                lines.push(format!("- [{label}]({rel}) \u{2014} {count}"));
            } else {
                lines.push(format!("- {label} \u{2014} 0"));
            }
        }
        lines.push(String::new());
        self.rewrite_body(okf_path, &format!("{}\n", lines.join("\n")), index_frontmatter("Code"))
    }

    /// Mirrors `CodeRenderer._render_arch_indexes`. Every entry in the
    /// Python `specs` list is its own ledger line: endpoints (via
    /// `_endpoint_line`), data models, schemas, and dependencies (all three
    /// via `_named_line`) — none skipped.
    fn render_arch_indexes(&self) -> Result<()> {
        let specs: [(&str, &str, &[Value], bool); 4] = [
            ("code/endpoints/index.md", "Endpoints", self.cb.endpoints.as_slice(), true),
            ("code/data-models/index.md", "Data Models", self.cb.data_models.as_slice(), false),
            ("code/schemas/index.md", "Schemas", self.cb.schemas.as_slice(), false),
            ("code/dependencies/index.md", "Dependencies", self.cb.dependencies.as_slice(), false),
        ];
        for (okf_path, title, records, is_endpoint) in specs {
            if records.is_empty() {
                continue;
            }
            let mut lines = vec![format!("# {title} ({})", records.len()), String::new()];
            for record in records {
                let rendered =
                    if is_endpoint { self.endpoint_line(okf_path, record) } else { self.named_line(okf_path, record) };
                if !rendered.is_empty() {
                    lines.push(rendered);
                }
            }
            lines.push(String::new());
            self.rewrite_body(okf_path, &format!("{}\n", lines.join("\n").trim_end()), index_frontmatter(title))?;
        }
        Ok(())
    }

    /// Mirrors `CodeRenderer._render_root_index`.
    fn render_root_index(&self) -> Result<()> {
        let title = self
            .manifest
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| bundle_dir_name(&self.bundle_dir));
        let lines = vec![
            format!("# {title}"),
            String::new(),
            "OKF code knowledge bundle. Start with the overview, then browse modules and symbols.".to_string(),
            String::new(),
            "- [Overview](overview.md) \u{2014} architecture narrative and diagrams".to_string(),
            "- [Request flow](request-flow.md) \u{2014} endpoint \u{2192} service \u{2192} repository \u{2192} model"
                .to_string(),
            format!(
                "- [Code](code/index.md) \u{2014} {} modules, {} symbols",
                self.cb.modules.len(),
                self.cb.symbols.len()
            ),
            format!("- [Sources](sources/index.md) \u{2014} {} files", self.cb.sources.len()),
            "- [Evidence](evidence/index.md) \u{2014} exact source spans".to_string(),
            "- [Relations](relations/index.md) \u{2014} the raw knowledge graph".to_string(),
            "- [Reports](reports/validation.md) \u{2014} validation, coverage, change impact".to_string(),
            "- [Agent guide](references/code-agent-guide.md)".to_string(),
            String::new(),
        ];
        self.rewrite_body("index.md", &format!("{}\n", lines.join("\n")), root_frontmatter(&title))
    }

    /// Mirrors `CodeRenderer._endpoint_line`, one of the two `line_fn`
    /// branches `_render_arch_indexes` dispatches to (the other being
    /// `_named_line`, just below).
    fn endpoint_line(&self, from_path: &str, ep: &Value) -> String {
        let method = ep.get("method").and_then(Value::as_str).unwrap_or("");
        let path = ep.get("path").and_then(Value::as_str).unwrap_or("");
        let label = format!("{method} {path}").trim().to_string();
        let ep_doc = self.endpoint_doc_path(ep);
        let label_md = match &ep_doc {
            Some(doc) => self.link(from_path, doc, &label),
            None => format!("`{label}`"),
        };
        if let Some(handler) = ep.get("handler_symbol_id").and_then(Value::as_str) {
            if self.cb.symbol_by_id.contains_key(handler) {
                return format!("- {label_md} \u{2192} {}", self.symbol_link(from_path, handler));
            }
        }
        format!("- {label_md} \u{2192} `{}`", py_get_str_default(ep, "handler_qname", ""))
    }

    /// Mirrors `CodeRenderer._named_line`.
    fn named_line(&self, from_path: &str, record: &Value) -> String {
        let name = record
            .get("title")
            .and_then(Value::as_str)
            .or_else(|| record.get("name").and_then(Value::as_str))
            .or_else(|| record.get("qualified_name").and_then(Value::as_str))
            .or_else(|| record.get("id").and_then(Value::as_str))
            .unwrap_or("");
        match record.get("okf_path").and_then(Value::as_str).filter(|s| !s.is_empty()) {
            Some(okf) => format!("- {}", self.link(from_path, okf, name)),
            None => format!("- `{name}`"),
        }
    }

    /// Mirrors `CodeRenderer.endpoint_doc_path`.
    pub fn endpoint_doc_path(&self, ep: &Value) -> Option<String> {
        let eid = ep.get("id").and_then(Value::as_str).filter(|s| !s.is_empty())?;
        let path = format!("code/endpoints/{eid}.md");
        if self.bundle_dir.join(&path).exists() {
            Some(path)
        } else {
            None
        }
    }

    /// Mirrors `CodeRenderer._calls_targets`.
    fn calls_targets(&self, symbol_id: &str) -> Vec<String> {
        self.cb
            .out_rel
            .get(symbol_id)
            .map(|idxs| {
                idxs.iter()
                    .filter_map(|&i| {
                        let rel = &self.cb.relations[i];
                        if rel.get("predicate").and_then(Value::as_str) != Some("calls") {
                            return None;
                        }
                        rel.get("object")
                            .and_then(Value::as_str)
                            .filter(|o| self.cb.symbol_by_id.contains_key(*o))
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Mirrors `CodeRenderer._reachable`, with the Python defaults
    /// (`max_depth=8, max_nodes=400`) baked in — no grounded call site ever
    /// overrides them.
    fn reachable(&self, start_id: &str) -> HashSet<String> {
        const MAX_DEPTH: usize = 8;
        const MAX_NODES: usize = 400;
        let mut seen: HashSet<String> = HashSet::new();
        let mut frontier: Vec<String> = vec![start_id.to_string()];
        let mut depth = 0;
        while !frontier.is_empty() && depth < MAX_DEPTH && seen.len() < MAX_NODES {
            let mut next: Vec<String> = Vec::new();
            for sid in &frontier {
                for target in self.calls_targets(sid) {
                    if target != start_id && !seen.contains(&target) {
                        seen.insert(target.clone());
                        next.push(target);
                    }
                }
            }
            frontier = next;
            depth += 1;
        }
        seen
    }

    /// Mirrors `CodeRenderer._layer`: "Classify a symbol into an
    /// architectural layer by module convention."
    fn layer(&self, symbol: &Value) -> &'static str {
        if let Some(id) = symbol.get("id").and_then(Value::as_str) {
            if self.model_symbol_ids.contains(id) {
                return "model";
            }
        }
        let module = symbol.get("module").and_then(Value::as_str).unwrap_or("");
        let parts: HashSet<&str> = module.split('.').collect();
        if parts.contains("models") {
            return "model";
        }
        if parts.contains("routes") || parts.contains("routers") {
            return "route";
        }
        if parts.contains("services") {
            return "service";
        }
        if parts.contains("repositories") || parts.iter().any(|p| p.ends_with("_repo")) {
            return "repo";
        }
        if parts.contains("schemas") {
            return "schema";
        }
        "other"
    }

    /// Mirrors `CodeRenderer._flow_cell`, with the Python default `limit=6`
    /// baked in (no grounded call site overrides it).
    fn flow_cell(&self, from_path: &str, symbols: &[Value]) -> String {
        const LIMIT: usize = 6;
        let mut unique: HashMap<String, Value> = HashMap::new();
        for s in symbols {
            if let Some(id) = s.get("id").and_then(Value::as_str) {
                unique.insert(id.to_string(), s.clone());
            }
        }
        let mut ordered: Vec<&Value> = unique.values().collect();
        ordered.sort_by_key(|s| s.get("qualified_name").and_then(Value::as_str).unwrap_or("").to_string());
        let mut items: Vec<String> = ordered
            .iter()
            .take(LIMIT)
            .map(|s| {
                let qname = s.get("qualified_name").and_then(Value::as_str).unwrap_or("");
                self.link(from_path, s.get("okf_path").and_then(Value::as_str).unwrap_or(""), &short_q(qname))
            })
            .collect();
        if ordered.len() > LIMIT {
            items.push(format!("+{}", ordered.len() - LIMIT));
        }
        if items.is_empty() {
            "\u{2014}".to_string()
        } else {
            items.join(", ")
        }
    }

    /// Mirrors `CodeRenderer._package_tree_data`: "Return (top-level package
    /// -> module count, top -> {two-level pkg -> count})."
    fn package_tree_data(&self) -> (std::collections::BTreeMap<String, usize>, HashMap<String, std::collections::BTreeMap<String, usize>>) {
        let mut top: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        let mut sub: HashMap<String, std::collections::BTreeMap<String, usize>> = HashMap::new();
        for module in &self.cb.modules {
            let qname = module.get("qualified_name").and_then(Value::as_str).unwrap_or("");
            let top_key = qname.split('.').next().unwrap_or("").to_string();
            let pkg_key = package_key(qname);
            *top.entry(top_key.clone()).or_insert(0) += 1;
            if pkg_key != top_key {
                let entry = sub.entry(top_key).or_default();
                *entry.entry(pkg_key).or_insert(0) += 1;
            }
        }
        (top, sub)
    }

    /// Mirrors `CodeRenderer._package_map_lines`.
    fn package_map_lines(&self) -> Vec<String> {
        let (top, sub) = self.package_tree_data();
        if top.is_empty() {
            return Vec::new();
        }
        let mut out = vec!["## Package map".to_string(), String::new()];
        let mut keys: Vec<&String> = top.keys().collect();
        keys.sort_by(|a, b| top[*b].cmp(&top[*a]).then_with(|| a.cmp(b)));
        for top_key in keys {
            out.push(format!("- `{top_key}` \u{2014} {} module(s)", top[top_key]));
            if let Some(inner) = sub.get(top_key) {
                for (pkg_key, count) in inner {
                    out.push(format!("  - `{pkg_key}` \u{2014} {count}"));
                }
            }
        }
        out.push(String::new());
        out
    }

    /// Mirrors `CodeRenderer._mermaid_package_tree`.
    fn mermaid_package_tree(&self) -> String {
        let (top, sub) = self.package_tree_data();
        if top.is_empty() {
            return String::new();
        }
        let root_label = self
            .manifest
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| bundle_dir_name(&self.bundle_dir));
        let mut lines =
            vec!["```mermaid".to_string(), "flowchart TD".to_string(), format!("  root[\"{}\"]", mm_label(&root_label))];
        let mut keys: Vec<&String> = top.keys().collect();
        keys.sort_by(|a, b| top[*b].cmp(&top[*a]).then_with(|| a.cmp(b)));
        for top_key in keys {
            let top_id = mm_id(&format!("pkg_{top_key}"));
            lines.push(format!("  root --> {top_id}[\"{} ({})\"]", mm_label(top_key), top[top_key]));
            if let Some(inner) = sub.get(top_key) {
                for (pkg_key, count) in inner {
                    let leaf = pkg_key.rsplit('.').next().unwrap_or(pkg_key);
                    lines.push(format!(
                        "  {top_id} --> {}[\"{} ({count})\"]",
                        mm_id(&format!("pkg_{pkg_key}")),
                        mm_label(leaf)
                    ));
                }
            }
        }
        lines.push("```".to_string());
        lines.join("\n")
    }

    /// Mirrors `CodeRenderer.package_tree_mermaid`.
    pub fn package_tree_mermaid(&self) -> String {
        self.mermaid_package_tree()
    }

    /// Mirrors `CodeRenderer.layered_flow_mermaid`.
    pub fn layered_flow_mermaid(&self) -> String {
        self.mermaid_layered_flow()
    }

    /// Mirrors `CodeRenderer.endpoint_flow_mermaid`.
    pub fn endpoint_flow_mermaid(&self, handler_symbol_id: &str) -> String {
        self.endpoint_flow_diagram(handler_symbol_id)
    }

    /// Mirrors `CodeRenderer._mermaid_layered_flow`: "Aggregated route ->
    /// service -> repository -> model call flow. Modules are nodes for the
    /// route/service/repo layers; model classes are nodes for the model
    /// layer. Only downstream `calls` edges are drawn." Ports the three
    /// nested closures (`key`, `nid`, `label`) as local closures.
    fn mermaid_layered_flow(&self) -> String {
        let rank: HashMap<&str, i32> = [("route", 0), ("service", 1), ("repo", 2), ("model", 3)].into_iter().collect();
        let key_fn = |sym: &Value| -> (String, String) {
            let layer = self.layer(sym);
            if layer == "model" {
                let qname = sym.get("qualified_name").and_then(Value::as_str).unwrap_or("");
                ("model".to_string(), qname.rsplit('.').next().unwrap_or(qname).to_string())
            } else {
                (layer.to_string(), sym.get("module").and_then(Value::as_str).unwrap_or("").to_string())
            }
        };

        let mut edges: BTreeSet<((String, String), (String, String))> = BTreeSet::new();
        let mut nodes: HashMap<&str, BTreeSet<(String, String)>> =
            ["route", "service", "repo", "model"].iter().map(|k| (*k, BTreeSet::new())).collect();

        for rel in &self.cb.relations {
            if rel.get("predicate").and_then(Value::as_str) != Some("calls") {
                continue;
            }
            let Some(a_id) = rel.get("subject").and_then(Value::as_str) else { continue };
            let Some(b_id) = rel.get("object").and_then(Value::as_str) else { continue };
            let Some(a) = self.symbol(a_id) else { continue };
            let Some(b) = self.symbol(b_id) else { continue };
            let la = self.layer(a);
            let lb = self.layer(b);
            let (Some(&ra), Some(&rb)) = (rank.get(la), rank.get(lb)) else { continue };
            if rb <= ra {
                continue;
            }
            let ka = key_fn(a);
            let kb = key_fn(b);
            edges.insert((ka.clone(), kb.clone()));
            nodes.get_mut(la).unwrap().insert(ka);
            nodes.get_mut(lb).unwrap().insert(kb);
        }
        if edges.is_empty() {
            return String::new();
        }

        let nid = |k: &(String, String)| mm_id(&format!("{}__{}", k.0, k.1));
        let label = |k: &(String, String)| mm_label(if k.0 != "model" { k.1.rsplit('.').next().unwrap_or(&k.1) } else { &k.1 });

        let mut lines = vec!["```mermaid".to_string(), "flowchart LR".to_string()];
        for (layer_key, title) in [("route", "Routes"), ("service", "Services"), ("repo", "Repositories"), ("model", "Models")] {
            let group = &nodes[layer_key];
            if group.is_empty() {
                continue;
            }
            lines.push(format!("  subgraph {title}"));
            for k in group {
                lines.push(format!("    {}[\"{}\"]", nid(k), label(k)));
            }
            lines.push("  end".to_string());
        }
        const CAP: usize = 150;
        for (a, b) in edges.iter().take(CAP) {
            lines.push(format!("  {} --> {}", nid(a), nid(b)));
        }
        if edges.len() > CAP {
            lines.push(format!("  %% {} more edges omitted", edges.len() - CAP));
        }
        lines.push("```".to_string());
        lines.join("\n")
    }

    /// Mirrors `CodeRenderer.render_request_flow`. Uses `write_markdown`
    /// directly with fixed frontmatter rather than `_rewrite_body`, matching
    /// the grounded Python source exactly (this is the one `render_*` method
    /// that does not preserve an existing file's frontmatter).
    pub fn render_request_flow(&self) -> Result<()> {
        if self.cb.endpoints.is_empty() {
            return Ok(());
        }
        let okf_path = "request-flow.md";
        let mut endpoints: Vec<&Value> = self.cb.endpoints.iter().collect();
        endpoints.sort_by_key(|e| {
            (
                e.get("router_prefix").and_then(Value::as_str).unwrap_or("").to_string(),
                e.get("path").and_then(Value::as_str).unwrap_or("").to_string(),
                e.get("method").and_then(Value::as_str).unwrap_or("").to_string(),
            )
        });
        let mut lines = vec![
            "# Request Flow".to_string(),
            String::new(),
            "Each HTTP endpoint traced through the layers it reaches, by following `calls` relations from the handler symbol: **route \u{2192} service \u{2192} repository \u{2192} model**. Derived statically; heuristic call resolution can miss dynamically dispatched calls.".to_string(),
            String::new(),
            "| Endpoint | Handler | Services | Repositories | Models |".to_string(),
            "|---|---|---|---|---|".to_string(),
        ];
        for ep in endpoints {
            let label_txt = format!(
                "{} {}",
                ep.get("method").and_then(Value::as_str).unwrap_or(""),
                ep.get("path").and_then(Value::as_str).unwrap_or("")
            )
            .trim()
            .to_string();
            let ep_doc = self.endpoint_doc_path(ep);
            let ep_label = match &ep_doc {
                Some(doc) => self.link(okf_path, doc, &label_txt),
                None => format!("`{label_txt}`"),
            };
            let handler_id = ep.get("handler_symbol_id").and_then(Value::as_str).filter(|h| self.cb.symbol_by_id.contains_key(*h));
            let Some(handler_id) = handler_id else {
                lines.push(format!(
                    "| {ep_label} | `{}` | \u{2014} | \u{2014} | \u{2014} |",
                    py_get_str_default(ep, "handler_qname", "")
                ));
                continue;
            };
            let reached = self.reachable(handler_id);
            let mut buckets: HashMap<&str, Vec<Value>> =
                [("service", Vec::new()), ("repo", Vec::new()), ("model", Vec::new())].into_iter().collect();
            for sid in &reached {
                if let Some(sym) = self.symbol(sid) {
                    let layer = self.layer(sym);
                    if let Some(bucket) = buckets.get_mut(layer) {
                        bucket.push(sym.clone());
                    }
                }
            }
            let handler = self.symbol(handler_id).expect("handler_id checked against symbol_by_id above");
            let handler_cell = self.link(
                okf_path,
                handler.get("okf_path").and_then(Value::as_str).unwrap_or(""),
                &short_q(handler.get("qualified_name").and_then(Value::as_str).unwrap_or("")),
            );
            lines.push(format!(
                "| {ep_label} | {handler_cell} | {} | {} | {} |",
                self.flow_cell(okf_path, &buckets["service"]),
                self.flow_cell(okf_path, &buckets["repo"]),
                self.flow_cell(okf_path, &buckets["model"]),
            ));
        }
        let mut fm = Map::new();
        fm.insert("type".into(), Value::String("Code Request Flow".into()));
        fm.insert("title".into(), Value::String("Request Flow".into()));
        write_markdown(&self.bundle_dir.join(okf_path), &Value::Object(fm), &format!("{}\n", lines.join("\n")))
    }

    /// Mirrors `CodeRenderer.render_endpoint_flows`: "Rewrite each endpoint
    /// doc with a per-request flow diagram + reached layers."
    pub fn render_endpoint_flows(&self) -> Result<()> {
        for ep in &self.cb.endpoints {
            let Some(okf_path) = self.endpoint_doc_path(ep) else { continue };
            let title = format!(
                "{} {}",
                ep.get("method").and_then(Value::as_str).unwrap_or(""),
                ep.get("path").and_then(Value::as_str).unwrap_or("")
            )
            .trim()
            .to_string();
            let mut fm = Map::new();
            fm.insert("type".into(), Value::String("Code Endpoint".into()));
            fm.insert("title".into(), Value::String(title));
            let body = self.endpoint_body(ep, &okf_path);
            self.rewrite_body(&okf_path, &body, Value::Object(fm))?;
        }
        Ok(())
    }

    /// Mirrors `CodeRenderer._endpoint_body`.
    fn endpoint_body(&self, ep: &Value, okf_path: &str) -> String {
        let method_path = format!(
            "{} {}",
            ep.get("method").and_then(Value::as_str).unwrap_or(""),
            ep.get("path").and_then(Value::as_str).unwrap_or("")
        )
        .trim()
        .to_string();
        let mut lines = vec![format!("# {method_path}"), String::new()];
        if let Some(prefix) = ep.get("router_prefix").and_then(Value::as_str).filter(|s| !s.is_empty()) {
            lines.push(format!("- Router: `{prefix}`"));
        }
        let handler_id = ep.get("handler_symbol_id").and_then(Value::as_str);
        let handler = handler_id.and_then(|h| self.symbol(h));
        let Some(handler) = handler else {
            lines.push(format!("- Handler: `{}`", py_get_str_default(ep, "handler_qname", "")));
            return format!("{}\n", lines.join("\n").trim_end());
        };
        let handler_id = handler_id.unwrap();
        lines.push(format!(
            "- Handler: {}",
            self.link(
                okf_path,
                handler.get("okf_path").and_then(Value::as_str).unwrap_or(""),
                handler.get("qualified_name").and_then(Value::as_str).unwrap_or("")
            )
        ));
        if let Some(file) = ep.get("file").and_then(Value::as_str).filter(|s| !s.is_empty()) {
            let line = value_or_default_str(ep.get("line"), "");
            lines.push(format!("- Source: `{file}:{line}`"));
        }
        lines.push(String::new());
        lines.push("## Flow".to_string());
        lines.push(String::new());
        let diagram = self.endpoint_flow_diagram(handler_id);
        if !diagram.is_empty() {
            lines.push(diagram);
            lines.push(String::new());
        } else {
            lines.push("_No downstream calls resolved for this handler._".to_string());
            lines.push(String::new());
        }

        let reached = self.reachable(handler_id);
        let mut buckets: HashMap<&str, Vec<Value>> =
            [("service", Vec::new()), ("repo", Vec::new()), ("model", Vec::new())].into_iter().collect();
        for sid in &reached {
            if let Some(sym) = self.symbol(sid) {
                let layer = self.layer(sym);
                if let Some(bucket) = buckets.get_mut(layer) {
                    bucket.push(sym.clone());
                }
            }
        }
        for (layer, title) in [("service", "Services"), ("repo", "Repositories"), ("model", "Models")] {
            let mut unique: HashMap<String, Value> = HashMap::new();
            for s in &buckets[layer] {
                if let Some(id) = s.get("id").and_then(Value::as_str) {
                    unique.insert(id.to_string(), s.clone());
                }
            }
            let mut group: Vec<&Value> = unique.values().collect();
            group.sort_by_key(|s| s.get("qualified_name").and_then(Value::as_str).unwrap_or("").to_string());
            if group.is_empty() {
                continue;
            }
            lines.push(format!("## {title}"));
            lines.push(String::new());
            for sym in group {
                lines.push(format!(
                    "- {}",
                    self.link(
                        okf_path,
                        sym.get("okf_path").and_then(Value::as_str).unwrap_or(""),
                        sym.get("qualified_name").and_then(Value::as_str).unwrap_or("")
                    )
                ));
            }
            lines.push(String::new());
        }
        format!("{}\n", lines.join("\n").trim_end())
    }

    /// Mirrors `CodeRenderer._endpoint_flow_diagram`: "Real per-request call
    /// subgraph (calls edges among app symbols reachable from the
    /// handler)."
    fn endpoint_flow_diagram(&self, handler_id: &str) -> String {
        let reached = self.reachable(handler_id);
        let mut node_ids: HashSet<String> = std::iter::once(handler_id.to_string())
            .chain(reached.into_iter().filter(|sid| self.cb.symbol_by_id.contains_key(sid)))
            .collect();
        // Slim very large graphs to the core layers so the diagram stays legible.
        if node_ids.len() > 60 {
            let core: HashSet<&str> = ["route", "service", "repo", "model"].into_iter().collect();
            node_ids = node_ids
                .into_iter()
                .filter(|sid| self.symbol(sid).map(|s| core.contains(self.layer(s))).unwrap_or(false))
                .collect();
            node_ids.insert(handler_id.to_string());
        }
        let mut edges: BTreeSet<(String, String)> = BTreeSet::new();
        for a in &node_ids {
            for t in self.calls_targets(a) {
                if node_ids.contains(&t) && &t != a {
                    edges.insert((a.clone(), t));
                }
            }
        }
        if edges.is_empty() {
            return String::new();
        }

        let order = [("route", "Route"), ("service", "Services"), ("repo", "Repositories"), ("model", "Models"), ("other", "Other")];
        let mut buckets: HashMap<&str, Vec<String>> = order.iter().map(|(k, _)| (*k, Vec::new())).collect();
        for sid in &node_ids {
            if let Some(sym) = self.symbol(sid) {
                let layer = self.layer(sym);
                let key = if buckets.contains_key(layer) { layer } else { "other" };
                buckets.get_mut(key).unwrap().push(sid.clone());
            }
        }
        let mut lines = vec!["```mermaid".to_string(), "flowchart LR".to_string()];
        for (key, title) in order {
            let mut group = buckets[key].clone();
            group.sort_by_key(|sid| {
                self.symbol(sid).and_then(|s| s.get("qualified_name")).and_then(Value::as_str).unwrap_or("").to_string()
            });
            if group.is_empty() {
                continue;
            }
            lines.push(format!("  subgraph {title}"));
            for sid in group {
                let qname = self.symbol(&sid).and_then(|s| s.get("qualified_name")).and_then(Value::as_str).unwrap_or("");
                lines.push(format!("    {}[\"{}\"]", mm_id(&sid), mm_label(&short_q(qname))));
            }
            lines.push("  end".to_string());
        }
        const CAP: usize = 120;
        for (a, b) in edges.iter().take(CAP) {
            lines.push(format!("  {} --> {}", mm_id(a), mm_id(b)));
        }
        if edges.len() > CAP {
            lines.push(format!("  %% {} more edges omitted", edges.len() - CAP));
        }
        lines.push("```".to_string());
        lines.join("\n")
    }

    /// Mirrors `CodeRenderer._entry_points`.
    fn entry_points(&self) -> Vec<String> {
        let okf_path = "overview.md";
        let special: HashSet<&str> = ["main", "app", "create_app", "lifespan"].into_iter().collect();
        let mut result: BTreeSet<String> = BTreeSet::new();
        for symbol in &self.cb.symbols {
            let name = symbol.get("title").and_then(Value::as_str).unwrap_or("");
            let module = symbol.get("module").and_then(Value::as_str).unwrap_or("");
            if special.contains(name) || module.ends_with(".main") {
                let id = symbol.get("id").and_then(Value::as_str).unwrap_or("");
                result.insert(format!("{} (`{module}`)", self.symbol_link(okf_path, id)));
            }
        }
        result.into_iter().take(12).collect()
    }

    /// Mirrors `CodeRenderer.render_overview`.
    pub fn render_overview(&self) -> Result<()> {
        let cb = &self.cb;
        let repo = &cb.repository;
        let title = self
            .manifest
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| bundle_dir_name(&self.bundle_dir));
        let langs = if cb.detected_languages.is_empty() {
            "n/a".to_string()
        } else {
            cb.detected_languages.iter().map(|(k, v)| format!("{k} ({})", value_display(v))).collect::<Vec<_>>().join(", ")
        };
        let frameworks = if cb.frameworks.is_empty() {
            "none detected".to_string()
        } else {
            cb.frameworks.iter().map(value_display).collect::<Vec<_>>().join(", ")
        };

        let mut lines = vec![format!("# {title} \u{2014} Overview"), String::new()];
        lines.extend([
            "| | |".to_string(),
            "|---|---|".to_string(),
            format!("| Languages | {langs} |"),
            format!("| Frameworks | {frameworks} |"),
            format!("| Modules | {} |", cb.modules.len()),
            format!("| Symbols | {} |", cb.symbols.len()),
            format!("| Endpoints | {} |", cb.endpoints.len()),
            format!("| Relations | {} |", cb.relations.len()),
            String::new(),
        ]);

        if let Some(path) = repo.get("path").and_then(Value::as_str).filter(|s| !s.is_empty()) {
            let mut vcs_parts: Vec<String> = Vec::new();
            for k in ["vcs", "branch"] {
                if let Some(v) = repo.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()) {
                    vcs_parts.push(v.to_string());
                }
            }
            let mut vcs_bits = vcs_parts.join(" ");
            let commit: String =
                repo.get("commit").and_then(Value::as_str).unwrap_or("").chars().take(12).collect();
            if !commit.is_empty() {
                vcs_bits = format!("{vcs_bits} @ {commit}").trim().to_string();
            }
            let suffix = if vcs_bits.is_empty() { String::new() } else { format!(" ({vcs_bits})") };
            lines.push(format!("Repository `{path}`{suffix}"));
            lines.push(String::new());
        }

        lines.extend(self.package_map_lines());

        let tree = self.mermaid_package_tree();
        if !tree.is_empty() {
            lines.push("## Package structure".to_string());
            lines.push(String::new());
            lines.push(tree);
            lines.push(String::new());
        }

        let entry_points = self.entry_points();
        if !entry_points.is_empty() {
            lines.push("## Entry points".to_string());
            lines.push(String::new());
            for label in entry_points {
                lines.push(format!("- {label}"));
            }
            lines.push(String::new());
        }

        if !cb.endpoints.is_empty() {
            lines.push(format!("## Request flow ({} endpoints)", cb.endpoints.len()));
            lines.push(String::new());
            lines.push("Layered call flow (route \u{2192} service \u{2192} repository \u{2192} model), aggregated across endpoints. See per-endpoint chains in [request-flow.md](request-flow.md).".to_string());
            lines.push(String::new());
            let flow = self.mermaid_layered_flow();
            if !flow.is_empty() {
                lines.push(flow);
                lines.push(String::new());
            }
        }

        if !cb.dependencies.is_empty() {
            lines.push("## External dependencies".to_string());
            lines.push(String::new());
            for dep in &cb.dependencies {
                let name = dep
                    .get("title")
                    .and_then(Value::as_str)
                    .or_else(|| dep.get("name").and_then(Value::as_str))
                    .or_else(|| dep.get("id").and_then(Value::as_str))
                    .unwrap_or("");
                lines.push(format!("- `{name}`"));
            }
            lines.push(String::new());
        }

        let mut fm = Map::new();
        fm.insert("type".into(), Value::String("Code Overview".into()));
        fm.insert("title".into(), Value::String(format!("{title} \u{2014} Overview")));
        write_markdown(
            &self.bundle_dir.join("overview.md"),
            &Value::Object(fm),
            &format!("{}\n", lines.join("\n").trim_end()),
        )
    }

    /// Mirrors `CodeRenderer.render`.
    pub fn render(&self) -> Result<Value> {
        self.render_symbol_bodies()?;
        self.render_module_bodies()?;
        self.render_indexes()?;
        self.render_overview()?;
        self.render_endpoint_flows()?;
        self.render_request_flow()?;
        let mut out = Map::new();
        out.insert("modules".into(), Value::from(self.cb.modules.len()));
        out.insert("symbols".into(), Value::from(self.cb.symbols.len()));
        out.insert("sources".into(), Value::from(self.cb.sources.len()));
        out.insert("endpoints".into(), Value::from(self.cb.endpoints.len()));
        out.insert("overview".into(), Value::String("overview.md".into()));
        Ok(Value::Object(out))
    }
}

/// Free-function form of `CodeRenderer._resolve_import`, used by the
/// constructor before `self` exists (see the doc comment on the instance
/// method `resolve_import`).
/// Mirrors Python's `d.get(key, default)` as embedded directly into an
/// f-string (e.g. `` f"...{ep.get('handler_qname', '')}..." ``), which is
/// subtly different from "missing-or-null -> default": the `default`
/// argument to `dict.get` only fires when `key` is *absent*. When the key
/// is present with an explicit JSON `null` (Python `None` — e.g. an
/// unresolved argparse CLI handler's `handler_qname`, which
/// `entrypoints.py::_named_raw` sets to `None` rather than a string when
/// `handlers.get(...)` found no match), `.get(key, default)` returns that
/// `None`, and f-string interpolation of `None` renders the literal text
/// `None` (via `str(None)`) — not the `default`. Fix (Low finding —
/// rendered unresolved handler): the previous version here used
/// `.and_then(Value::as_str).unwrap_or(default)`, which collapses *both*
/// "key absent" and "key present but null" to `default`, so an unresolved
/// CLI handler rendered as an empty string instead of Python's literal
/// `None` text. Confirmed by reading the concrete source path: a `None`
/// `handler_qname` only actually reaches `.get(key, '')` default-swallowing
/// in `_write_endpoint_docs`'s body line (`f"- Handler: {e.get('handler_qname')
/// or '—'}"`, which uses `or`, not `.get(key, default)`, so that one spot
/// legitimately shows `'—'`) vs. `CodeRenderer._endpoint_line`/`_render_arch_indexes`'s
/// index lines (`f"...{ep.get('handler_qname', '')}..."`), which do show
/// literal `None` for this case.
fn py_get_str_default<'a>(v: &'a Value, key: &str, default: &'a str) -> std::borrow::Cow<'a, str> {
    match v.get(key) {
        None => std::borrow::Cow::Borrowed(default),
        Some(Value::Null) => std::borrow::Cow::Borrowed("None"),
        Some(Value::String(s)) => std::borrow::Cow::Borrowed(s.as_str()),
        Some(other) => std::borrow::Cow::Owned(other.to_string()),
    }
}

fn resolve_import_for(cb: &CodeBundle, modules_by_qname: &HashMap<String, Value>, import_name: &str, importing_module: &str) -> Option<Value> {
    if let Some(target) = resolve_import_target(import_name, &cb.modules, importing_module) {
        return Some(target);
    }
    let parts: Vec<&str> = import_name.split('.').collect();
    for cut in (1..parts.len()).rev() {
        let candidate = parts[..cut].join(".");
        if let Some(found) =
            modules_by_qname.get(&candidate).cloned().or_else(|| resolve_import_target(&candidate, &cb.modules, importing_module))
        {
            return Some(found);
        }
    }
    None
}

/// Mirrors `kl4a.codekb.render._short_signature`.
fn short_signature(signature: Option<&str>) -> Option<String> {
    let signature = signature.filter(|s| !s.is_empty())?;
    let collapsed = WHITESPACE_RE.replace_all(signature, " ").trim().to_string();
    if collapsed.chars().count() > MAX_SIG {
        let truncated: String = collapsed.chars().take(MAX_SIG).collect();
        Some(format!("{} \u{2026}", truncated.trim_end()))
    } else {
        Some(collapsed)
    }
}

/// Mirrors `kl4a.codekb.render._first_paragraph`.
fn first_paragraph(text: &str) -> String {
    for block in text.split("\n\n") {
        let block = block.trim();
        if !block.is_empty() {
            return block.to_string();
        }
    }
    text.trim().to_string()
}

/// Mirrors `kl4a.codekb.render._index_frontmatter`.
fn index_frontmatter(title: &str) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), Value::String("Code Bundle Index".into()));
    m.insert("title".into(), Value::String(title.into()));
    Value::Object(m)
}

/// Mirrors `kl4a.codekb.render._module_frontmatter`.
fn module_frontmatter(module: &Value) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), Value::String("Code Module".into()));
    m.insert("title".into(), module.get("title").cloned().unwrap_or(Value::Null));
    m.insert("module_id".into(), module.get("id").cloned().unwrap_or(Value::Null));
    Value::Object(m)
}

/// Mirrors `kl4a.codekb.render._symbol_frontmatter`.
fn symbol_frontmatter(symbol: &Value) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), Value::String("Code Symbol".into()));
    m.insert("title".into(), symbol.get("title").cloned().unwrap_or(Value::Null));
    m.insert("symbol_id".into(), symbol.get("id").cloned().unwrap_or(Value::Null));
    Value::Object(m)
}

/// Mirrors `kl4a.codekb.render._root_frontmatter`.
fn root_frontmatter(title: &str) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), Value::String("Code Knowledge Bundle".into()));
    m.insert("title".into(), Value::String(title.into()));
    m.insert("okf_version".into(), Value::String("0.2".into()));
    m.insert("profile".into(), Value::String("code-knowledge-bundle".into()));
    Value::Object(m)
}

/// Mirrors `kl4a.codekb.render._mm_id`.
fn mm_id(value: &str) -> String {
    let cleaned: String = value.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let cleaned = cleaned.trim_matches('_');
    if cleaned.is_empty() { "node".to_string() } else { cleaned.to_string() }
}

/// Mirrors `kl4a.codekb.render._mm_label`.
fn mm_label(value: &str) -> String {
    value.replace('"', "'").replace('[', "(").replace(']', ")")
}

/// Mirrors `kl4a.codekb.render._package_key`: "Two-level package for a
/// module: first two segments when the module is nested (e.g.
/// `app.services`), else the single top-level name (`tests`, `main`)."
fn package_key(qualified_name: &str) -> String {
    let parts: Vec<&str> = qualified_name.split('.').collect();
    if parts.len() >= 3 {
        parts[..2].join(".")
    } else {
        parts.first().copied().unwrap_or("").to_string()
    }
}

/// Mirrors `kl4a.codekb.render._short_q`.
fn short_q(qualified_name: &str) -> String {
    let parts: Vec<&str> = qualified_name.split('.').collect();
    let start = parts.len().saturating_sub(2);
    parts[start..].join(".")
}

fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut cap_next = true;
    for c in s.chars() {
        if c.is_alphabetic() {
            if cap_next {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            cap_next = false;
        } else {
            out.push(c);
            cap_next = true;
        }
    }
    out
}

fn bundle_dir_name(bundle_dir: &Path) -> String {
    bundle_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// Python `str(x)` applied to a relation's `object`/`subject` field, which
/// may already be a string but is not guaranteed to be.
fn value_to_key_string(v: &Value) -> String {
    v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())
}

/// Python `f"{value}"` where `value` is a bare `dict.get(key)` (no default),
/// so a missing key formats as the literal string `"None"`.
fn value_display_opt(v: Option<&Value>) -> String {
    match v {
        Some(val) => value_display(val),
        None => "None".to_string(),
    }
}

/// Python `f"{value}"` where `value` came from `dict.get(key, default)` (an
/// explicit, already-typed default used verbatim when the key is missing).
fn value_or_default_str(v: Option<&Value>, default: &str) -> String {
    match v {
        Some(val) => value_display(val),
        None => default.to_string(),
    }
}

fn value_display(v: &Value) -> String {
    match v {
        Value::Null => "None".to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => {
            if *b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        other => other.to_string(),
    }
}
