//! Port of `kl4a/codekb/bundle.py`.
//!
//! Creates/validates the Code Knowledge Bundle directory skeleton and its
//! `manifest.yaml`, and writes the bundle-root/section `index.md` docs.
//! Ported symbol-for-symbol from the Python source (verified via
//! `tools-code` MCP `code_symbols_get`).
//!
//! Cross-batch dependencies not yet present in this crate (see the final
//! report's ledger for exact required signatures):
//! - `crate::state` (port of `kl4a/codekb/state.py`): `STATE_DIR`,
//!   `read_json`, `write_json`, `save_manifest` (atomic).
//! - `crate::config` (port of `kl4a/codekb/config.py`): `effective_codekb_config`.
//!
//! `manifest.yaml` writes in this file go through `crate::state::save_manifest`,
//! which is atomic (temp file + rename) — grounded via MCP evidence for
//! `kl4a.codekb.state.save_manifest`/`write_text_atomic`, and load-bearing per
//! this port's stated requirement ("a reader polling during a write never
//! sees a torn manifest"). `write_code_index`'s per-section `index.md` docs
//! are NOT written atomically, matching the Python original (plain
//! `write_markdown`) — they are regenerated docs, not state a concurrent
//! reader depends on being whole.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use once_cell::sync::Lazy;
use serde_json::{json, Value};

use crate::bundle_store::{is_falsy, load_manifest, utc_now};
use crate::ids::slugify;
use crate::okf_writer::write_markdown;

/// Port of `kl4a.codekb.bundle.CODE_REQUIRED_DIRS` (`bundle.py:17-40`).
///
/// ```python
/// CODE_REQUIRED_DIRS = [
///     "sources", "sources/repositories", "sources/files", "sources/commits",
///     "sources/originals", "code", "code/modules", "code/symbols",
///     "code/endpoints", "code/data-models", "code/schemas", "code/dependencies",
///     "concepts", "knowledge", "relations", "evidence", "tasks", "references",
///     "reports", STATE_DIR, f"{STATE_DIR}/cache", f"{STATE_DIR}/indexes",
/// ]
/// ```
///
/// `STATE_DIR`-derived entries are computed lazily from `crate::state::STATE_DIR`
/// (out of scope for this batch) rather than hardcoded, so the two stay in sync.
pub static CODE_REQUIRED_DIRS: Lazy<Vec<String>> = Lazy::new(|| {
    let mut dirs: Vec<String> = [
        "sources",
        "sources/repositories",
        "sources/files",
        "sources/commits",
        "sources/originals",
        "code",
        "code/modules",
        "code/symbols",
        "code/endpoints",
        "code/data-models",
        "code/schemas",
        "code/dependencies",
        "concepts",
        "knowledge",
        "relations",
        "evidence",
        "tasks",
        "references",
        "reports",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    dirs.push(crate::state::STATE_DIR.to_string());
    dirs.push(format!("{}/cache", crate::state::STATE_DIR));
    dirs.push(format!("{}/indexes", crate::state::STATE_DIR));
    dirs
});

/// Port of `kl4a.codekb.bundle.apply_mining_mode` (`bundle.py:99-115`).
///
/// ```python
/// def apply_mining_mode(codekb: dict[str, Any], mining_mode: str | None) -> dict[str, Any]:
///     """Record the static/hybrid choice in the bundle's codekb config.
///
///     Mining mode is operational policy, so it lives in configuration and is chosen
///     once per bundle rather than being re-supplied on every command.
///     """
///     if not mining_mode:
///         return codekb
///     if mining_mode not in {"static", "hybrid", "llm"}:
///         raise ValueError(f"unsupported mining mode: {mining_mode}")
///     codekb = dict(codekb)
///     mining = dict(codekb.get("mining") or {})
///     mining["mode"] = "hybrid" if mining_mode in {"hybrid", "llm"} else "static"
///     codekb["mining"] = mining
///     return codekb
/// ```
pub fn apply_mining_mode(codekb: &Value, mining_mode: Option<&str>) -> Result<Value> {
    let mining_mode = match mining_mode {
        Some(mode) if !mode.is_empty() => mode,
        _ => return Ok(codekb.clone()),
    };
    if !matches!(mining_mode, "static" | "hybrid" | "llm") {
        bail!("unsupported mining mode: {mining_mode}");
    }
    let mut codekb = codekb.as_object().cloned().unwrap_or_default();
    let mut mining = codekb
        .get("mining")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mode = if matches!(mining_mode, "hybrid" | "llm") {
        "hybrid"
    } else {
        "static"
    };
    mining.insert("mode".to_string(), json!(mode));
    codekb.insert("mining".to_string(), Value::Object(mining));
    Ok(Value::Object(codekb))
}

/// Port of `kl4a.codekb.bundle.rel_link` (`bundle.py:208-217`).
///
/// ```python
/// def rel_link(from_okf_path: str, to_okf_path: str) -> str:
///     """Return a POSIX, ``..``-aware relative link from one bundle doc to another. ..."""
///     from_dir = posixpath.dirname(from_okf_path.replace("\\", "/"))
///     return posixpath.relpath(to_okf_path.replace("\\", "/"), from_dir or ".")
/// ```
///
/// Reimplemented purely on path *strings*, not `std::path`, because both
/// arguments are bundle-root-relative POSIX *document* paths (e.g.
/// `code/symbols/x.md`), never real filesystem paths — going through
/// `std::path::Path` on Windows would apply Windows path semantics to what
/// is really just a virtual OKF link target.
pub fn rel_link(from_okf_path: &str, to_okf_path: &str) -> String {
    let normalized_from = from_okf_path.replace('\\', "/");
    let from_dir = posix_dirname(&normalized_from);
    let start = if from_dir.is_empty() { "." } else { &from_dir };
    posix_relpath(&to_okf_path.replace('\\', "/"), start)
}

/// `posixpath.dirname` — the directory component of a POSIX path string.
fn posix_dirname(path: &str) -> String {
    match path.rfind('/') {
        None => String::new(),
        Some(idx) => {
            let head = &path[..=idx];
            if head.chars().all(|c| c == '/') {
                head.to_string()
            } else {
                head.trim_end_matches('/').to_string()
            }
        }
    }
}

/// `posixpath.relpath(path, start)`, restricted to the case this codebase
/// actually exercises: both `path` and `start` are clean, `..`-free,
/// bundle-relative POSIX paths (no leading `/`). UNCONFIRMED: a real
/// `..`-containing input is not normalized the way `posixpath.abspath` would;
/// none of this port's callers pass one.
fn posix_relpath(path: &str, start: &str) -> String {
    fn split(value: &str) -> Vec<&str> {
        value.split('/').filter(|part| !part.is_empty() && *part != ".").collect()
    }
    let start_parts = split(start);
    let path_parts = split(path);
    let common = start_parts
        .iter()
        .zip(path_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut rel: Vec<&str> = Vec::new();
    for _ in 0..(start_parts.len() - common) {
        rel.push("..");
    }
    rel.extend_from_slice(&path_parts[common..]);
    if rel.is_empty() {
        ".".to_string()
    } else {
        rel.join("/")
    }
}

/// Port of `kl4a.codekb.bundle.validate_code_profile` (`bundle.py:187-201`).
///
/// ```python
/// def validate_code_profile(bundle_dir: Path) -> tuple[list[str], list[str]]:
///     errors: list[str] = []
///     warnings: list[str] = []
///     try:
///         manifest = load_manifest(bundle_dir)
///     except Exception as exc:
///         return [str(exc)], warnings
///     if manifest.get("profile") != "code-knowledge-bundle":
///         errors.append("manifest profile must be code-knowledge-bundle")
///     if not manifest.get("codekb"):
///         errors.append("manifest missing codekb configuration")
///     for relative in CODE_REQUIRED_DIRS:
///         if not (bundle_dir / relative).is_dir():
///             errors.append(f"missing required code directory: {relative}")
///     return errors, warnings
/// ```
pub fn validate_code_profile(bundle_dir: &Path) -> (Vec<String>, Vec<String>) {
    let warnings: Vec<String> = Vec::new();
    let manifest = match load_manifest(bundle_dir) {
        Ok(manifest) => manifest,
        Err(err) => return (vec![err.to_string()], warnings),
    };
    let mut errors: Vec<String> = Vec::new();
    if manifest.get("profile").and_then(Value::as_str) != Some("code-knowledge-bundle") {
        errors.push("manifest profile must be code-knowledge-bundle".to_string());
    }
    if is_falsy(manifest.get("codekb")) {
        errors.push("manifest missing codekb configuration".to_string());
    }
    for relative in CODE_REQUIRED_DIRS.iter() {
        if !bundle_dir.join(relative).is_dir() {
            errors.push(format!("missing required code directory: {relative}"));
        }
    }
    (errors, warnings)
}

/// Port of `kl4a.codekb.bundle.write_code_state` (`bundle.py:204-205`).
///
/// ```python
/// def write_code_state(bundle_dir: Path, filename: str, data: Any) -> None:
///     write_json(bundle_dir / STATE_DIR / filename, data)
/// ```
///
/// Reconciled per the coordinator's integration pass: `state.rs` (a sibling
/// batch) independently ported this same function byte-for-byte identically
/// (its own doc comment flagged the same duplicate and suggested this exact
/// fix). Kept as a re-export here rather than two copies of the same body,
/// so `crate::bundle::write_code_state` and `crate::state::write_code_state`
/// are guaranteed to never drift apart.
pub use crate::state::write_code_state;

/// Port of `kl4a.codekb.bundle.write_code_index` (`bundle.py:118-184`).
///
/// Writes `index.md` at the bundle root, one `index.md` per top-level
/// section directory, and `references/code-agent-guide.md`. See the Python
/// source for each document's exact frontmatter/body; reproduced verbatim
/// below via the grounded MCP evidence excerpt.
pub fn write_code_index(bundle_dir: &Path, manifest: &Value) -> Result<()> {
    let title = manifest.get("title").and_then(Value::as_str).unwrap_or("");
    let okf_version = manifest
        .get("okf_version")
        .and_then(Value::as_str)
        .unwrap_or("0.2");
    let profile = manifest.get("profile").and_then(Value::as_str).unwrap_or("");

    write_markdown(
        &bundle_dir.join("index.md"),
        &json!({
            "type": "Code Knowledge Bundle",
            "title": title,
            "okf_version": okf_version,
            "profile": profile,
        }),
        &[
            format!("# {title}"),
            String::new(),
            "This OKF bundle contains source-code knowledge represented as files, modules, symbols, evidence, relations, and agent task contexts.".to_string(),
            String::new(),
            "- [Sources](sources/index.md)".to_string(),
            "- [Code](code/index.md)".to_string(),
            "- [Evidence](evidence/index.md)".to_string(),
            "- [Relations](relations/index.md)".to_string(),
            "- [Tasks](tasks/index.md)".to_string(),
            "- [Reports](reports/validation.md)".to_string(),
            String::new(),
        ]
        .join("\n"),
    )?;

    let section_indexes: [(&str, &str); 12] = [
        ("sources", "Code Sources"),
        ("sources/repositories", "Repositories"),
        ("sources/files", "Source Files"),
        ("code", "Code"),
        ("code/modules", "Modules"),
        ("code/symbols", "Symbols"),
        ("evidence", "Evidence"),
        ("relations", "Relations"),
        ("knowledge", "Knowledge"),
        ("concepts", "Concepts"),
        ("tasks", "Agent Task Contexts"),
        ("references", "References"),
    ];
    for (directory, section_title) in section_indexes {
        write_markdown(
            &bundle_dir.join(directory).join("index.md"),
            &json!({"type": "Code Bundle Index", "title": section_title}),
            &format!("# {section_title}\n"),
        )?;
    }

    write_markdown(
        &bundle_dir.join("references").join("code-agent-guide.md"),
        &json!({"type": "Code Agent Guide", "title": "Code Agent Guide"}),
        &[
            "# Code Agent Guide".to_string(),
            String::new(),
            "Developer agents should consume this bundle through the Code Knowledge Access Layer.".to_string(),
            String::new(),
            "Recommended flow:".to_string(),
            String::new(),
            "1. Describe the repository with `code.repo.describe`.".to_string(),
            "2. Search files and symbols with `code.files.search` and `code.symbols.search`.".to_string(),
            "3. Retrieve task context with `code.context` before planning edits.".to_string(),
            "4. Resolve evidence handles before making code claims.".to_string(),
            "5. Treat Tier 4 claims as excluded unless reviewed for operational use.".to_string(),
            String::new(),
        ]
        .join("\n"),
    )
}

/// Port of `kl4a.codekb.bundle.create_code_bundle` (`bundle.py:43-96`).
///
/// ```python
/// def create_code_bundle(bundle_dir: Path, repo_dir: Path, *, title: str | None = None,
///                         mining_mode: str | None = None) -> dict[str, Any]:
///     if not str(repo_dir).strip():
///         raise ValueError("a repository folder path is required")
///     repo_dir = repo_dir.expanduser()
///     if not repo_dir.exists():
///         raise ValueError(f"repository folder does not exist: {repo_dir}")
///     if not repo_dir.is_dir():
///         raise ValueError(f"repository path is not a folder: {repo_dir}")
///
///     bundle_dir.mkdir(parents=True, exist_ok=True)
///     for relative in CODE_REQUIRED_DIRS:
///         (bundle_dir / relative).mkdir(parents=True, exist_ok=True)
///
///     if (bundle_dir / "manifest.yaml").exists():
///         manifest = load_manifest(bundle_dir)
///         if mining_mode:
///             manifest["codekb"] = apply_mining_mode(manifest.get("codekb") or {}, mining_mode)
///             manifest["updated_at"] = utc_now()
///             save_manifest(bundle_dir, manifest)
///         return manifest
///
///     timestamp = utc_now()
///     repo_name = repo_dir.resolve().name
///     manifest = {
///         "id": slugify(bundle_dir.name), "version": "0.1.0",
///         "title": title or f"{repo_name} Code Knowledge Bundle",
///         "profile": "code-knowledge-bundle", "profile_version": "0.1.0",
///         "okf_version": "0.2", "status": "draft",
///         "created_at": timestamp, "updated_at": timestamp,
///         "sources": [], "exports": [],
///         "codekb": apply_mining_mode(effective_codekb_config(repo_dir), mining_mode),
///     }
///     save_manifest(bundle_dir, manifest)
///     write_code_index(bundle_dir, manifest)
///     return manifest
/// ```
///
/// `save_manifest` here is `kl4a.codekb.state.save_manifest` — **atomic**
/// (temp file + rename), on both the re-init branch and the fresh-bundle
/// branch. This is the hard, load-bearing requirement this batch was told to
/// preserve: a reader polling `manifest.yaml` mid-write must see either the
/// whole old file or the whole new one, never a torn one.
pub fn create_code_bundle(
    bundle_dir: &Path,
    repo_dir: &Path,
    title: Option<&str>,
    mining_mode: Option<&str>,
) -> Result<Value> {
    if repo_dir.as_os_str().is_empty() || repo_dir.to_string_lossy().trim().is_empty() {
        bail!("a repository folder path is required");
    }
    let repo_dir = expand_user(repo_dir);
    if !repo_dir.exists() {
        bail!("repository folder does not exist: {}", repo_dir.display());
    }
    if !repo_dir.is_dir() {
        bail!("repository path is not a folder: {}", repo_dir.display());
    }

    std::fs::create_dir_all(bundle_dir)?;
    // Long-path safety: re-derive `bundle_dir` as its canonical (Windows
    // extended-length-capable) form for every subsequent join/write in this
    // function, now that the top-level directory is guaranteed to exist.
    // Only a stripped display string (via `bundle_store::display_path`) is
    // ever written into a manifest/JSON field; the raw canonical `PathBuf`
    // here is purely for on-disk correctness on deeply nested paths.
    let bundle_dir = &bundle_dir.canonicalize().unwrap_or_else(|_| bundle_dir.to_path_buf());
    for relative in CODE_REQUIRED_DIRS.iter() {
        std::fs::create_dir_all(bundle_dir.join(relative))?;
    }

    let manifest_path = bundle_dir.join("manifest.yaml");
    if manifest_path.exists() {
        let mut manifest = load_manifest(bundle_dir)?;
        if let Some(mode) = mining_mode.filter(|m| !m.is_empty()) {
            let existing_codekb = manifest
                .get("codekb")
                .filter(|v| !is_falsy(Some(v)))
                .cloned()
                .unwrap_or(json!({}));
            let updated_codekb = apply_mining_mode(&existing_codekb, Some(mode))?;
            if let Value::Object(map) = &mut manifest {
                map.insert("codekb".to_string(), updated_codekb);
                map.insert("updated_at".to_string(), json!(utc_now()));
            }
            // UNCONFIRMED WIRING: `crate::state::save_manifest` (atomic),
            // out of scope for this batch (`kl4a/codekb/state.py`).
            crate::state::save_manifest(bundle_dir, &manifest)?;
        }
        return Ok(manifest);
    }

    let timestamp = utc_now();
    let canonical_repo_dir = repo_dir.canonicalize().unwrap_or_else(|_| repo_dir.clone());
    let repo_name = canonical_repo_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let bundle_name = bundle_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    // UNCONFIRMED WIRING: `effective_codekb_config` is `kl4a.codekb.config.
    // effective_codekb_config`, out of scope for this batch (`crate::config`).
    let codekb_config = crate::config::effective_codekb_config(&repo_dir, None)?;
    let codekb = apply_mining_mode(&Value::Object(codekb_config), mining_mode)?;

    let manifest = json!({
        "id": slugify(&bundle_name),
        "version": "0.1.0",
        "title": title.map(str::to_string).unwrap_or_else(|| format!("{repo_name} Code Knowledge Bundle")),
        "profile": "code-knowledge-bundle",
        "profile_version": "0.1.0",
        "okf_version": "0.2",
        "status": "draft",
        "created_at": timestamp,
        "updated_at": timestamp,
        "sources": [],
        "exports": [],
        "codekb": codekb,
    });
    crate::state::save_manifest(bundle_dir, &manifest)?;
    write_code_index(bundle_dir, &manifest)?;
    Ok(manifest)
}

/// `Path.expanduser()` — expands a leading `~` (current user only) using
/// `HOME` (Unix) / `USERPROFILE` (Windows). UNCONFIRMED: Python's version
/// also handles `~otheruser` on Unix via `pwd`; that form is left untouched
/// here, matching only the "current user's own home" case this codebase's
/// callers actually hit.
fn expand_user(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(rest) = raw.strip_prefix('~') {
        if rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\') {
            if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
                let mut expanded = PathBuf::from(home);
                let rest_trimmed = rest.trim_start_matches(['/', '\\']);
                if !rest_trimmed.is_empty() {
                    expanded.push(rest_trimmed);
                }
                return expanded;
            }
        }
    }
    path.to_path_buf()
}
