//! Port of `kl4a/codekb/cache.py`.
//!
//! Response cache for one bundle's LLM/author enrichment calls, loaded once
//! per mining run. Ported symbol-for-symbol from the Python source (verified
//! via `tools-code` MCP `code_symbols_get`).
//!
//! Depends on `crate::state` (port of `kl4a/codekb/state.py` — `STATE_DIR`,
//! `read_json`, `write_json`) which is **out of scope for this batch** and is
//! not yet present in this crate. See the completeness ledger in this batch's
//! final report for the exact symbols that module must expose.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::bundle_store::utc_now;
// UNCONFIRMED WIRING: `kl4a/codekb/state.py` is a separate source file, not
// part of this batch (bundle.py/cache.py/canonical.py/inventory.py only).
// `STATE_DIR`, `read_json`, `write_json` are grounded (see final report) but
// have no Rust home yet. Referencing the expected sibling module here so
// this file is ready to compile the moment `state.rs` lands.
use crate::state::{read_json, write_json, STATE_DIR};

/// `kl4a.codekb.cache.CACHE_FILENAME` (`cache.py:33`).
pub const CACHE_FILENAME: &str = "code_enrichment.json";

/// `kl4a.codekb.cache.CACHE_VERSION` (`cache.py:34`).
pub const CACHE_VERSION: u64 = 1;

/// Port of `kl4a.codekb.cache.cache_path` (`cache.py:37-40`).
///
/// ```python
/// def cache_path(bundle_dir: Path, *, filename: str = CACHE_FILENAME) -> Path:
///     # .codekb/cache/ is already excluded from the canonical OKF document set...
///     return bundle_dir / STATE_DIR / "cache" / filename
/// ```
pub fn cache_path(bundle_dir: &Path, filename: &str) -> PathBuf {
    bundle_dir.join(STATE_DIR).join("cache").join(filename)
}

/// Port of `kl4a.codekb.cache.fingerprint` (`cache.py:43-54`).
///
/// ```python
/// def fingerprint(request: dict[str, Any], *, actor: str, prompt: str) -> str:
///     """Identify one question, so a changed question is a different entry."""
///     payload = json.dumps(
///         {
///             "version": CACHE_VERSION,
///             "request": request,
///             "actor": actor,
///             "prompt": hashlib.sha256(prompt.encode("utf-8")).hexdigest(),
///         },
///         sort_keys=True,
///     )
///     return hashlib.sha256(payload.encode("utf-8")).hexdigest()
/// ```
///
/// `serde_json::Map` in this workspace is a `BTreeMap` (no `preserve_order`
/// feature enabled), so keys already serialize in sorted order at every
/// nesting level, matching `sort_keys=True`. UNCONFIRMED / not bit-for-bit:
/// `serde_json::to_string`'s separators (`,`/`:`, no spaces) differ from
/// Python `json.dumps`'s default (`, `/`: `), so a fingerprint computed here
/// will *not* equal one computed by the Python original for the same input —
/// this only matters if a Rust and Python build are ever expected to share
/// one bundle's cache file, which is not a stated requirement for this port.
pub fn fingerprint(request: &Value, actor: &str, prompt: &str) -> String {
    let prompt_digest = {
        let mut hasher = Sha256::new();
        hasher.update(prompt.as_bytes());
        format!("{:x}", hasher.finalize())
    };
    let payload = json!({
        "version": CACHE_VERSION,
        "request": request,
        "actor": actor,
        "prompt": prompt_digest,
    });
    let serialized = serde_json::to_string(&payload).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Port of `kl4a.codekb.cache.clear_cache` (`cache.py:157-163`).
///
/// ```python
/// def clear_cache(bundle_dir: Path) -> bool:
///     """Remove the cache entirely. Returns whether there was anything to remove."""
///     path = cache_path(bundle_dir)
///     if not path.exists():
///         return False
///     path.unlink()
///     return True
/// ```
pub fn clear_cache(bundle_dir: &Path) -> Result<bool> {
    let path = cache_path(bundle_dir, CACHE_FILENAME);
    if !path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&path)?;
    Ok(true)
}

/// One cached response, keyed by [`fingerprint`] in [`EnrichmentCache::entries`].
///
/// `dict[str, Any]`-shaped in Python (`{"response", "symbol_id",
/// "source_version_id", "stored_at"}`); given a real struct here since the
/// shape is stable and always constructed the same way by [`EnrichmentCache::put`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub response: Value,
    pub symbol_id: String,
    pub source_version_id: Option<String>,
    pub stored_at: String,
}

/// `kl4a.codekb.cache.EnrichmentCache.stats` return shape (`cache.py:147-154`).
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub enabled: bool,
    pub hits: u64,
    pub misses: u64,
    pub pruned: u64,
    pub entries: usize,
}

/// Port of `kl4a.codekb.cache.EnrichmentCache` (`cache.py:57-154`).
///
/// ```python
/// class EnrichmentCache:
///     """Response cache for one bundle, loaded once per mining run.
///
///     A disabled cache is still constructed, so callers need no conditionals; it
///     simply never reports a hit and never persists.
///     """
/// ```
pub struct EnrichmentCache {
    pub bundle_dir: PathBuf,
    pub enabled: bool,
    pub filename: String,
    pub entries: Map<String, Value>,
    pub hits: u64,
    pub misses: u64,
    pub pruned: u64,
}

impl EnrichmentCache {
    /// Port of `EnrichmentCache.__init__` (`cache.py:64-77`).
    ///
    /// ```python
    /// def __init__(
    ///     self, bundle_dir: Path, *, enabled: bool = True, filename: str = CACHE_FILENAME
    /// ) -> None:
    ///     self.bundle_dir = bundle_dir
    ///     self.enabled = enabled
    ///     self.filename = filename
    ///     self.entries: dict[str, Any] = {}
    ///     self.hits = 0
    ///     self.misses = 0
    ///     self.pruned = 0
    ///     if enabled:
    ///         stored = read_json(cache_path(bundle_dir, filename=filename), {})
    ///         if isinstance(stored, dict) and stored.get("version") == CACHE_VERSION:
    ///             self.entries = dict(stored.get("entries") or {})
    /// ```
    pub fn new(bundle_dir: &Path, enabled: bool, filename: &str) -> Self {
        let mut entries = Map::new();
        if enabled {
            let stored = read_json(&cache_path(bundle_dir, filename), json!({}));
            if let Value::Object(stored_map) = &stored {
                let version_matches = stored_map
                    .get("version")
                    .and_then(Value::as_u64)
                    .map(|v| v == CACHE_VERSION)
                    .unwrap_or(false);
                if version_matches {
                    if let Some(Value::Object(stored_entries)) = stored_map.get("entries") {
                        entries = stored_entries.clone();
                    }
                }
            }
        }
        Self {
            bundle_dir: bundle_dir.to_path_buf(),
            enabled,
            filename: filename.to_string(),
            entries,
            hits: 0,
            misses: 0,
            pruned: 0,
        }
    }

    /// Convenience constructor matching the Python default keyword args
    /// (`enabled=True, filename=CACHE_FILENAME`).
    pub fn open(bundle_dir: &Path) -> Self {
        Self::new(bundle_dir, true, CACHE_FILENAME)
    }

    /// Port of `EnrichmentCache.get` (`cache.py:79-87`).
    ///
    /// ```python
    /// def get(self, key: str) -> dict[str, Any] | None:
    ///     if not self.enabled:
    ///         return None
    ///     entry = self.entries.get(key)
    ///     if entry is None:
    ///         self.misses += 1
    ///         return None
    ///     self.hits += 1
    ///     return entry.get("response")
    /// ```
    pub fn get(&mut self, key: &str) -> Option<Value> {
        if !self.enabled {
            return None;
        }
        match self.entries.get(key) {
            None => {
                self.misses += 1;
                None
            }
            Some(entry) => {
                self.hits += 1;
                entry.get("response").cloned()
            }
        }
    }

    /// Port of `EnrichmentCache.put` (`cache.py:89-104`).
    ///
    /// ```python
    /// def put(
    ///     self, key: str, response: dict[str, Any], *, symbol_id: str,
    ///     source_version_id: str | None,
    /// ) -> None:
    ///     if not self.enabled:
    ///         return
    ///     self.entries[key] = {
    ///         "response": response,
    ///         "symbol_id": symbol_id,
    ///         "source_version_id": source_version_id,
    ///         "stored_at": utc_now(),
    ///     }
    /// ```
    pub fn put(
        &mut self,
        key: &str,
        response: Value,
        symbol_id: &str,
        source_version_id: Option<&str>,
    ) {
        if !self.enabled {
            return;
        }
        self.entries.insert(
            key.to_string(),
            json!({
                "response": response,
                "symbol_id": symbol_id,
                "source_version_id": source_version_id,
                "stored_at": utc_now(),
            }),
        );
    }

    /// Port of `EnrichmentCache.prune` (`cache.py:106-133`).
    ///
    /// ```python
    /// def prune(self, live_symbol_ids: set[str]) -> None:
    ///     """Drop entries that can never be hit again. ..."""
    ///     if not self.enabled:
    ///         return
    ///     newest: dict[str, tuple[str, dict[str, Any]]] = {}
    ///     for key, entry in self.entries.items():
    ///         symbol_id = str(entry.get("symbol_id") or "")
    ///         if symbol_id not in live_symbol_ids:
    ///             continue
    ///         current = newest.get(symbol_id)
    ///         if current is None or str(entry.get("stored_at") or "") >= str(
    ///             current[1].get("stored_at") or ""
    ///         ):
    ///             newest[symbol_id] = (key, entry)
    ///     keep = {key: entry for key, entry in newest.values()}
    ///     self.pruned = len(self.entries) - len(keep)
    ///     self.entries = keep
    /// ```
    pub fn prune(&mut self, live_symbol_ids: &HashSet<String>) {
        if !self.enabled {
            return;
        }
        // (key, entry) of the newest surviving entry per symbol_id.
        let mut newest: std::collections::HashMap<String, (String, Value)> =
            std::collections::HashMap::new();
        for (key, entry) in self.entries.iter() {
            let symbol_id = entry
                .get("symbol_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if symbol_id.is_empty() || !live_symbol_ids.contains(&symbol_id) {
                continue;
            }
            let stored_at = entry.get("stored_at").and_then(Value::as_str).unwrap_or("");
            let replace = match newest.get(&symbol_id) {
                None => true,
                Some((_, current)) => {
                    let current_stored_at =
                        current.get("stored_at").and_then(Value::as_str).unwrap_or("");
                    stored_at >= current_stored_at
                }
            };
            if replace {
                newest.insert(symbol_id, (key.clone(), entry.clone()));
            }
        }
        let mut keep = Map::new();
        for (key, entry) in newest.into_values() {
            keep.insert(key, entry);
        }
        self.pruned = (self.entries.len() - keep.len()) as u64;
        self.entries = keep;
    }

    /// Port of `EnrichmentCache.save` (`cache.py:135-145`).
    ///
    /// ```python
    /// def save(self) -> None:
    ///     if not self.enabled:
    ///         return
    ///     write_json(
    ///         cache_path(self.bundle_dir, filename=self.filename),
    ///         {"version": CACHE_VERSION, "updated_at": utc_now(), "entries": self.entries},
    ///     )
    /// ```
    pub fn save(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let payload = json!({
            "version": CACHE_VERSION,
            "updated_at": utc_now(),
            "entries": Value::Object(self.entries.clone()),
        });
        write_json(&cache_path(&self.bundle_dir, &self.filename), &payload)
    }

    /// Port of `EnrichmentCache.stats` (`cache.py:147-154`).
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            enabled: self.enabled,
            hits: self.hits,
            misses: self.misses,
            pruned: self.pruned,
            entries: self.entries.len(),
        }
    }
}
