//! Port of `kl4a/codekb/model.py`.
//!
//! `CodeBundle` is a thin lazy-loading wrapper around a code-knowledge bundle
//! directory. Python's index maps (`module_by_id`, `symbol_by_id`,
//! `evidence_by_id`, `item_by_id`) hold references to the *same* dict objects
//! that live in the parallel lists (`modules`, `symbols`, `evidence`,
//! `items`), so a later in-place mutation (e.g. `_load_reviews` patching
//! `review_status` onto an item) is visible through both the map and the
//! list at once. Rust has no free aliasing like that for owned `Value`s, so
//! the maps here store indices into the corresponding `Vec` instead of
//! clones; mutating `items[idx]` and then reading it back through
//! `item_by_id` reproduces the same observable behavior.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use serde_json::{Map, Value};

use crate::state::STATE_DIR;

// WIRING: `load_manifest` lives on the Python side in `kl4a.kl4a.bundle_store`
// (a shared package above codekb/apikb), not inside `kl4a/codekb`. This batch
// was scoped to `kl4a/codekb/{knowledge,layout,lifecycle,model}.py` only, so
// `load_manifest` was not ported here. It needs a home -- most likely a
// `kl4a-core` crate (or a `crate::bundle_store` module in one of the existing
// crates) that both `codekb` and `apikb` depend on. Until that lands this
// reference will not compile; the coordinator should wire the real path in
// during lib.rs assembly.
use crate::bundle_store::load_manifest;

/// Mirrors `kl4a.codekb.model.CodeBundle`.
pub struct CodeBundle {
    pub dir: PathBuf,
    pub state_dir: PathBuf,

    pub repository: Map<String, Value>,
    pub detected_languages: Map<String, Value>,
    pub sources: Vec<Value>,

    pub modules: Vec<Value>,
    pub symbols: Vec<Value>,
    pub evidence: Vec<Value>,

    pub relations: Vec<Value>,
    pub rel_summary: Map<String, Value>,

    pub items: Vec<Value>,
    pub know_summary: Map<String, Value>,

    pub module_by_id: HashMap<String, usize>,
    pub symbol_by_id: HashMap<String, usize>,
    pub evidence_by_id: HashMap<String, usize>,
    pub item_by_id: HashMap<String, usize>,

    /// relation index lists keyed by relation `subject`
    pub out_rel: HashMap<String, Vec<usize>>,
    /// relation index lists keyed by relation `object`
    pub in_rel: HashMap<String, Vec<usize>>,

    /// item index lists keyed by each symbol id the item names in `symbols`
    pub items_by_symbol: HashMap<String, Vec<usize>>,
    /// symbol index lists keyed by `symbol["file"]`
    pub symbols_by_source: HashMap<String, Vec<usize>>,

    /// out-degree + in-degree per symbol id, from `out_rel`/`in_rel`
    pub degree: HashMap<String, usize>,

    pub arch: Value,
    pub endpoints: Vec<Value>,
    pub routers: Vec<Value>,
    pub data_models: Vec<Value>,
    pub schemas: Vec<Value>,
    pub dependencies: Vec<Value>,
    pub frameworks: Vec<Value>,

    pub review_status: Map<String, Value>,
    pub review_events: Vec<Value>,
}

impl CodeBundle {
    /// Mirrors `CodeBundle.__init__`.
    pub fn new(bundle_dir: &Path) -> Result<Self> {
        let state_dir = bundle_dir.join(STATE_DIR);

        let inv = load_state_file(&state_dir, "code_inventory.json")?;
        let sym = load_state_file(&state_dir, "code_symbols.json")?;
        let rel = load_state_file(&state_dir, "code_relations.json")?;
        let know = load_state_file(&state_dir, "code_knowledge.json")?;

        let repository = obj_field(&inv, "repository");
        let detected_languages = obj_field(&inv, "detected_languages");
        let sources = arr_field(&inv, "sources");

        let modules = arr_field(&sym, "modules");
        let symbols = arr_field(&sym, "symbols");
        let evidence = arr_field(&sym, "evidence");

        let relations = arr_field(&rel, "relations");
        let rel_summary = obj_field(&rel, "summary");

        let items = arr_field(&know, "items");
        let know_summary = obj_field(&know, "summary");

        let module_by_id = index_by_id(&modules);
        let symbol_by_id = index_by_id(&symbols);
        let evidence_by_id = index_by_id(&evidence);
        let item_by_id = index_by_id(&items);

        let mut out_rel: HashMap<String, Vec<usize>> = HashMap::new();
        let mut in_rel: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, r) in relations.iter().enumerate() {
            if let Some(s) = r.get("subject").and_then(Value::as_str) {
                out_rel.entry(s.to_string()).or_default().push(idx);
            }
            if let Some(o) = r.get("object").and_then(Value::as_str) {
                in_rel.entry(o.to_string()).or_default().push(idx);
            }
        }

        let mut items_by_symbol: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, it) in items.iter().enumerate() {
            if let Some(sids) = it.get("symbols").and_then(Value::as_array) {
                for sid in sids {
                    if let Some(s) = sid.as_str() {
                        items_by_symbol.entry(s.to_string()).or_default().push(idx);
                    }
                }
            }
        }

        let mut symbols_by_source: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, s) in symbols.iter().enumerate() {
            let file = s
                .get("file")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            symbols_by_source.entry(file).or_default().push(idx);
        }

        let mut degree: HashMap<String, usize> = HashMap::new();
        for s in &symbols {
            if let Some(id) = s.get("id").and_then(Value::as_str) {
                let out_n = out_rel.get(id).map(|v| v.len()).unwrap_or(0);
                let in_n = in_rel.get(id).map(|v| v.len()).unwrap_or(0);
                degree.insert(id.to_string(), out_n + in_n);
            }
        }

        let arch = load_state_file(&state_dir, "code_architecture.json")?;
        let endpoints = arr_field(&arch, "endpoints");
        let routers = arr_field(&arch, "routers");
        let data_models = arr_field(&arch, "data_models");
        let schemas = arr_field(&arch, "schemas");
        let dependencies = arr_field(&arch, "dependencies");
        let frameworks = arr_field(&arch, "frameworks");

        let mut bundle = CodeBundle {
            dir: bundle_dir.to_path_buf(),
            state_dir,
            repository,
            detected_languages,
            sources,
            modules,
            symbols,
            evidence,
            relations,
            rel_summary,
            items,
            know_summary,
            module_by_id,
            symbol_by_id,
            evidence_by_id,
            item_by_id,
            out_rel,
            in_rel,
            items_by_symbol,
            symbols_by_source,
            degree,
            arch,
            endpoints,
            routers,
            data_models,
            schemas,
            dependencies,
            frameworks,
            review_status: Map::new(),
            review_events: Vec::new(),
        };
        bundle.load_reviews()?;
        Ok(bundle)
    }

    /// Mirrors `CodeBundle.review_path`.
    pub fn review_path(&self) -> PathBuf {
        self.state_dir.join("code_reviews.json")
    }

    /// Mirrors `CodeBundle._load_reviews`.
    fn load_reviews(&mut self) -> Result<()> {
        let path = self.review_path();
        let data: Value = if path.exists() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            serde_json::from_str(&text)
                .with_context(|| format!("parsing {}", path.display()))?
        } else {
            Value::Object(Map::new())
        };
        self.review_status = data
            .get("status")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        self.review_events = data
            .get("events")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (iid, status) in self.review_status.clone() {
            if let Some(&idx) = self.item_by_id.get(&iid) {
                if let Some(obj) = self.items[idx].as_object_mut() {
                    obj.insert("review_status".to_string(), status);
                }
            }
        }
        Ok(())
    }
}

/// Mirrors `CodeBundle._load`: read `<state_dir>/<name>`, `{}` if missing.
/// A malformed file's parse error propagates, matching `json.loads` in the
/// original (no try/except around it there).
fn load_state_file(state_dir: &Path, name: &str) -> Result<Value> {
    let path = state_dir.join(name);
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let value: Value = serde_json::from_str(&text)
        .with_context(|| format!("parsing {}", path.display()))?;
    Ok(value)
}

fn obj_field(v: &Value, key: &str) -> Map<String, Value> {
    v.get(key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn arr_field(v: &Value, key: &str) -> Vec<Value> {
    v.get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn index_by_id(items: &[Value]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for (idx, item) in items.iter().enumerate() {
        if let Some(id) = item.get("id").and_then(Value::as_str) {
            map.insert(id.to_string(), idx);
        }
    }
    map
}

/// Process-wide mtime-keyed cache. Mirrors `kl4a.codekb.model._CACHE`
/// (`dict[str, tuple[float, CodeBundle]]`): keyed on the resolved bundle
/// directory, the value pairs the mtime of `code_relations.json` at load
/// time with the cached bundle, so a bundle that changes on disk (a re-mine,
/// a re-parse) invalidates the cached instance instead of being served stale
/// forever. Other modules may rely on this invalidation-on-mtime-change
/// behavior, so it is kept exactly rather than simplified to a plain lookup.
static CACHE: Lazy<Mutex<HashMap<PathBuf, (SystemTime, Arc<CodeBundle>)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Mirrors `get_code_bundle`.
pub fn get_code_bundle(bundle_dir: &Path) -> Result<Arc<CodeBundle>> {
    let key = bundle_dir
        .canonicalize()
        .unwrap_or_else(|_| bundle_dir.to_path_buf());
    let rel_path = bundle_dir.join(STATE_DIR).join("code_relations.json");
    // Python: `rel.stat().st_mtime if rel.exists() else 0.0`.
    let mtime = rel_path
        .metadata()
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    {
        let cache = CACHE.lock().expect("CACHE mutex poisoned");
        if let Some((cached_mtime, cb)) = cache.get(&key) {
            if *cached_mtime == mtime {
                return Ok(Arc::clone(cb));
            }
        }
    }

    let cb = Arc::new(CodeBundle::new(bundle_dir)?);
    let mut cache = CACHE.lock().expect("CACHE mutex poisoned");
    cache.insert(key, (mtime, Arc::clone(&cb)));
    Ok(cb)
}

/// Mirrors `web.py`'s `_CACHE.pop(str(bundle_dir), None)` calls after a
/// mutation (`handle_code_pipeline_form`/`handle_code_review_form`), so the
/// next page load re-reads the bundle from disk instead of serving a stale
/// cached `CodeBundle`. `model.py`'s own `_CACHE` had no public pop/evict
/// entry point for other modules to call, so this is a new export on the
/// Rust side of the same static -- not a Python symbol with its own name,
/// but the same observable "drop this key" effect `web.py` relies on.
pub fn invalidate_code_bundle_cache(bundle_dir: &Path) {
    let key = bundle_dir
        .canonicalize()
        .unwrap_or_else(|_| bundle_dir.to_path_buf());
    let mut cache = CACHE.lock().expect("CACHE mutex poisoned");
    cache.remove(&key);
}

/// Mirrors `is_code_bundle`.
///
/// Identifies a code bundle by its manifest profile first (set at creation,
/// so a freshly created bundle is recognised before anything has been
/// parsed); the `code_symbols.json` existence check is kept as a fallback
/// for bundles written before the profile was recorded.
pub fn is_code_bundle(bundle_dir: &Path) -> bool {
    if let Ok(manifest) = load_manifest(bundle_dir) {
        let profile = manifest
            .get("profile")
            .and_then(Value::as_str)
            .unwrap_or("");
        if profile == "code-knowledge-bundle" {
            return true;
        }
    }
    bundle_dir
        .join(STATE_DIR)
        .join("code_symbols.json")
        .exists()
}

/// Mirrors `_bundle_state`: a status that varies, rather than the bundle's
/// type restated.
fn bundle_state(cb: &CodeBundle) -> String {
    if cb.symbols.is_empty() {
        return "not built".to_string();
    }
    let pending = cb
        .items
        .iter()
        .filter(|item| {
            let lifecycle_status = falsy_or(item.get("lifecycle_status"), "active");
            let review_required = truthy(item.get("review_required"))
                || item.get("knowledge_tier").and_then(Value::as_i64) == Some(4);
            let proposed = item.get("review_status").and_then(Value::as_str) == Some("proposed");
            lifecycle_status == "active" && review_required && proposed
        })
        .count();
    if pending > 0 {
        format!("{pending} awaiting review")
    } else {
        "reviewed".to_string()
    }
}

/// Mirrors `code_bundle_describe`.
pub fn code_bundle_describe(bundle_dir: &Path) -> Result<Value> {
    let cb = get_code_bundle(bundle_dir)?;
    let fallback_name = bundle_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let id = cb
        .repository
        .get("id")
        .cloned()
        .unwrap_or_else(|| Value::String(fallback_name.clone()));
    let title = cb
        .repository
        .get("title")
        .cloned()
        .unwrap_or_else(|| Value::String(fallback_name));

    let mut out = Map::new();
    out.insert("id".into(), id);
    out.insert("title".into(), title);
    out.insert("status".into(), Value::String(bundle_state(&cb)));
    out.insert("source_count".into(), Value::from(cb.sources.len()));
    out.insert("symbol_count".into(), Value::from(cb.symbols.len()));
    out.insert("relation_count".into(), Value::from(cb.relations.len()));
    out.insert("knowledge_item_count".into(), Value::from(cb.items.len()));
    out.insert("unit".into(), Value::String("claims".into()));
    Ok(Value::Object(out))
}

/// Python `x.get(key) or default`: falls back on a *falsy* value (missing,
/// null, or empty string), not just a missing key.
fn falsy_or<'a>(value: Option<&'a Value>, default: &'a str) -> &'a str {
    match value.and_then(Value::as_str) {
        Some(s) if !s.is_empty() => s,
        _ => default,
    }
}

/// Python truthiness for a JSON-ish value pulled out of a `dict.get(...)`.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None => false,
        Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}
