//! Port of `kl4a/kl4a/hashing.py`. Grounded via `tools-code` MCP
//! `code_symbols_get` (`symbol-kl4a-kl4a-hashing-sha256-file`).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

/// Port of `kl4a.kl4a.hashing.sha256_file` (`kl4a/kl4a/hashing.py:14-19`).
///
/// ```python
/// def sha256_file(path: Path) -> str:
///     digest = hashlib.sha256()
///     with path.open("rb") as handle:
///         for chunk in iter(lambda: handle.read(1024 * 1024), b""):
///             digest.update(chunk)
///     return f"sha256:{digest.hexdigest()}"
/// ```
///
/// Reads the whole file rather than chunking (Rust's `fs::read` already
/// buffers internally); the observable result — the hex digest — is
/// identical.
pub fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}
