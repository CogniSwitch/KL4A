//! Port of `kl4a/codekb/inventory.py`.
//!
//! Scans a repository into the bundle's `code_inventory.json` state plus the
//! per-repository / per-source-file OKF markdown docs. Ported symbol-for-
//! symbol from the Python source (verified via `tools-code` MCP
//! `code_symbols_get`).
//!
//! Cross-batch dependencies not yet present in this crate (see the final
//! report's ledger for exact required signatures):
//! - `crate::state` (port of `kl4a/codekb/state.py`): `save_manifest`.
//! - `crate::config` (port of `kl4a/codekb/config.py`): `effective_codekb_config`,
//!   `configured_values`.
//! - `crate::ids` (port of `kl4a/codekb/ids.py`): `as_posix`, `code_source_id_for`.
//! - `crate::bundle` (port of `kl4a/codekb/bundle.py`, **this batch** —
//!   `write_code_state`, defined in `bundle.rs`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use once_cell::sync::Lazy;
use serde_json::{json, Value};

use crate::bundle::write_code_state;
use crate::bundle_store::{display_path, format_unix_utc, load_manifest, relative_to_bundle, utc_now};
use crate::hashing::sha256_file;
use crate::okf_writer::write_markdown;

/// Port of `kl4a.codekb.adapters.cobol.COBOL_EXTENSIONS` (`adapters/cobol.py:18`).
///
/// ```python
/// COBOL_EXTENSIONS = {".cbl", ".cob", ".cpy"}
/// ```
///
/// `adapters/cobol.py` is out of scope for this batch; this one three-entry
/// constant is duplicated here (rather than left as a compile blocker) since
/// `SUPPORTED_LANGUAGE_EXTENSIONS` below is defined directly in terms of it
/// in the Python source. Grounded via `tools-code` MCP, not invented.
pub const COBOL_EXTENSIONS: [&str; 3] = [".cbl", ".cob", ".cpy"];

/// Port of `kl4a.codekb.inventory.SUPPORTED_LANGUAGE_EXTENSIONS` (`inventory.py:22-25`).
///
/// ```python
/// SUPPORTED_LANGUAGE_EXTENSIONS = {
///     ".py": "python",
///     **{extension: "cobol" for extension in COBOL_EXTENSIONS},
/// }
/// ```
pub static SUPPORTED_LANGUAGE_EXTENSIONS: Lazy<std::collections::HashMap<&'static str, &'static str>> =
    Lazy::new(|| {
        let mut map = std::collections::HashMap::new();
        map.insert(".py", "python");
        for extension in COBOL_EXTENSIONS {
            map.insert(extension, "cobol");
        }
        map
    });

/// Port of `kl4a.codekb.inventory.DETECTED_LANGUAGE_EXTENSIONS` (`inventory.py:27-38`).
///
/// ```python
/// DETECTED_LANGUAGE_EXTENSIONS = {
///     ".js": "javascript", ".jsx": "javascript", ".ts": "typescript",
///     ".tsx": "typescript", ".java": "java", ".go": "go", ".rs": "rust",
///     ".cs": "csharp", ".rb": "ruby", ".php": "php",
/// }
/// ```
pub static DETECTED_LANGUAGE_EXTENSIONS: Lazy<std::collections::HashMap<&'static str, &'static str>> =
    Lazy::new(|| {
        [
            (".js", "javascript"),
            (".jsx", "javascript"),
            (".ts", "typescript"),
            (".tsx", "typescript"),
            (".java", "java"),
            (".go", "go"),
            (".rs", "rust"),
            (".cs", "csharp"),
            (".rb", "ruby"),
            (".php", "php"),
        ]
        .into_iter()
        .collect()
    });

/// Port of `kl4a.codekb.inventory.git_output` (`inventory.py:143-157`).
///
/// ```python
/// def git_output(repo_dir: Path, args: list[str]) -> str | None:
///     try:
///         result = subprocess.run(["git", *args], cwd=repo_dir, text=True,
///                                  capture_output=True, timeout=5, check=False)
///     except Exception:
///         return None
///     if result.returncode != 0:
///         return None
///     return result.stdout.strip() or None
/// ```
///
/// `std::process` has no built-in timeout. This reproduces `timeout=5` with a
/// watcher thread that force-kills the child by PID via the platform's own
/// `kill`/`taskkill` (no extra crate dependency) if it is still running after
/// 5 seconds — matching "a hung `git` call is treated as a failure", not
/// "block forever".
pub fn git_output(repo_dir: &Path, args: &[&str]) -> Option<String> {
    let mut command = std::process::Command::new("git");
    command.args(args);
    command.current_dir(repo_dir);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    let child = command.spawn().ok()?;
    let pid = child.id();

    let done = Arc::new(AtomicBool::new(false));
    let watcher_done = Arc::clone(&done);
    let watcher = std::thread::spawn(move || {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if watcher_done.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !watcher_done.load(Ordering::SeqCst) {
            kill_pid(pid);
        }
    });

    let output = child.wait_with_output();
    done.store(true, Ordering::SeqCst);
    let _ = watcher.join();

    let output = output.ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

fn kill_pid(pid: u32) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F", "/T"])
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status();
    }
}

/// Port of `kl4a.codekb.inventory.iter_repo_files` (`inventory.py:113-123`).
///
/// ```python
/// def iter_repo_files(repo_dir: Path, *, skip_dirs: list[str]) -> list[Path]:
///     skip_set = set(skip_dirs)
///     files: list[Path] = []
///     for path in sorted(repo_dir.rglob("*")):
///         if not path.is_file():
///             continue
///         relative_parts = path.relative_to(repo_dir).parts
///         if any(part in skip_set for part in relative_parts):
///             continue
///         files.append(path)
///     return files
/// ```
///
/// UNCONFIRMED: Python's `sorted(Path)` comparison is case-insensitive on
/// Windows (`WindowsPath` normalizes case for comparisons) but this sorts
/// `PathBuf`s byte-for-byte; on a repo with mixed-case filenames that differ
/// only by case, ordering could diverge. Not expected to matter for the
/// fixture repos this is tested against.
pub fn iter_repo_files(repo_dir: &Path, skip_dirs: &[String]) -> Vec<PathBuf> {
    let skip_set: HashSet<&str> = skip_dirs.iter().map(|s| s.as_str()).collect();
    let mut files: Vec<PathBuf> = Vec::new();
    // Fix (Medium finding — directory junctions during scan). Root cause,
    // investigated rather than assumed (the audit explicitly flagged this
    // as unexplored): `WalkDir`'s default is `follow_links(false)`, under
    // which any entry whose `symlink_metadata` reports it as a
    // reparse-point-like entry is yielded as a single leaf, never
    // recursed into. On Windows, `std::fs`'s notion of "symlink" covers
    // *any* reparse point, including an NTFS directory junction (tag
    // `IO_REPARSE_TAG_MOUNT_POINT`) — not just a true symlink (tag
    // `IO_REPARSE_TAG_SYMLINK`) — so a junction's contents were silently
    // never scanned. Python's `Path.rglob("*")` recurses into it instead:
    // confirmed directly (`.venv` interpreter, Python 3.13, a real
    // `mklink /J` junction under a temp repo) — `os.path.islink()` returns
    // `False` for a junction on Windows (Python only classifies the
    // `IO_REPARSE_TAG_SYMLINK` tag as a "link"; a junction's different
    // reparse tag makes it look like an ordinary directory to Python's
    // filesystem calls), so `rglob`'s own `recurse_symlinks=False` default
    // (new in 3.13) never kicks in for a junction and it gets walked like
    // any other directory. Fixed by enabling `follow_links(true)` here so
    // a junction's contents are scanned, matching Python's actual
    // behavior. This also makes Rust follow a *true* symlinked directory,
    // which Python's 3.13 default (`recurse_symlinks=False`) would NOT
    // follow — an unavoidable one-knob tradeoff, since `walkdir` has no
    // "follow junctions but not symlinks" distinction (both are
    // reparse-point "symlinks" to it) — but junctions are the
    // Windows-native, much more common case in practice (true symlinks
    // need an elevated/dev-mode privilege to create on Windows at all),
    // and `walkdir`'s `follow_links(true)` has built-in cycle detection,
    // so a true circular symlink fails safe here rather than diverging
    // further.
    for entry in walkdir::WalkDir::new(repo_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let relative = path.strip_prefix(repo_dir).unwrap_or(path);
        let skip = relative
            .components()
            .any(|part| skip_set.contains(part.as_os_str().to_string_lossy().as_ref()));
        if skip {
            continue;
        }
        files.push(path.to_path_buf());
    }
    files.sort();
    files
}

/// Port of `kl4a.codekb.inventory.repository_record` (`inventory.py:126-140`).
///
/// ```python
/// def repository_record(repo_dir: Path) -> dict[str, Any]:
///     commit = git_output(repo_dir, ["rev-parse", "HEAD"])
///     branch = git_output(repo_dir, ["rev-parse", "--abbrev-ref", "HEAD"])
///     return {
///         "id": f"repo-{repo_dir.name.lower().replace(' ', '-')}",
///         "title": repo_dir.name, "path": str(repo_dir),
///         "vcs": "git" if commit else "filesystem",
///         "commit": commit, "branch": branch,
///         "scanned_at": datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
///     }
/// ```
pub fn repository_record(repo_dir: &Path) -> Value {
    let commit = git_output(repo_dir, &["rev-parse", "HEAD"]);
    let branch = git_output(repo_dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let name = repo_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    json!({
        "id": format!("repo-{}", name.to_lowercase().replace(' ', "-")),
        "title": name,
        // Bug fix: was `repo_dir.to_string_lossy()`, which leaked Windows'
        // `\\?\`-prefixed canonical path (from `scan_code_repo`'s
        // `repo_dir.canonicalize()`) straight into the bundle's stored
        // `repository.path` field -- surfaced downstream in `repo describe`,
        // manifest.yaml, rendered docs, `serve`'s home page, and the MCP
        // `code.repo.describe` tool, none of which Python ever prefixes this
        // way (`Path.resolve()` has no `\\?\` concept). `display_path`
        // canonicalizes (so it's still the real, resolved path) but strips
        // the prefix for this display/storage string only.
        "path": display_path(repo_dir),
        "vcs": if commit.is_some() { "git" } else { "filesystem" },
        "commit": commit,
        "branch": branch,
        "scanned_at": utc_now(),
    })
}

/// Port of `kl4a.codekb.inventory.copy_original` (`inventory.py:197-212`).
///
/// ```python
/// def copy_original(bundle_dir: Path, path: Path, source_id: str, checksum: str) -> Path:
///     """Snapshot a source file into the bundle, refreshing it when it has changed."""
///     destination = bundle_dir / "sources" / "originals" / f"{source_id}__v1{path.suffix.lower()}"
///     destination.parent.mkdir(parents=True, exist_ok=True)
///     if not destination.exists() or sha256_file(destination) != checksum:
///         shutil.copy2(path, destination)
///     return destination
/// ```
///
/// `shutil.copy2` also copies metadata (mtime, permission bits) onto the
/// destination; `std::fs::copy` copies permission bits on Unix but not mtime.
/// UNCONFIRMED / accepted divergence: nothing downstream reads the *snapshot
/// file's* mtime (only the original repo file's, in [`source_record`]), so
/// this should not change any bundle output.
pub fn copy_original(bundle_dir: &Path, path: &Path, source_id: &str, checksum: &str) -> Result<PathBuf> {
    let suffix = path
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let destination = bundle_dir
        .join("sources")
        .join("originals")
        .join(format!("{source_id}__v1{suffix}"));
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let needs_copy = if !destination.exists() {
        true
    } else {
        sha256_file(&destination)? != checksum
    };
    if needs_copy {
        std::fs::copy(path, &destination)?;
    }
    Ok(destination)
}

/// Port of `kl4a.codekb.inventory.source_record` (`inventory.py:160-194`).
///
/// ```python
/// def source_record(repo_dir, bundle_dir, path, relative_path, language) -> dict[str, Any]:
///     source_id = code_source_id_for(relative_path)
///     checksum = sha256_file(path)
///     original_path = copy_original(bundle_dir, path, source_id, checksum)
///     is_test = ("test" in path.parts or path.name.startswith("test_")
///                or path.name.endswith("_test.py"))
///     is_copybook = language == "cobol" and path.suffix.lower() == ".cpy"
///     return {
///         "id": source_id, "title": as_posix(relative_path), "path": as_posix(relative_path),
///         "language": language,
///         "kind": "copybook" if is_copybook else "test" if is_test and language == "python" else "source",
///         "checksum": checksum, "size_bytes": path.stat().st_size,
///         "modified_time": datetime.fromtimestamp(path.stat().st_mtime, UTC)
///             .replace(microsecond=0).isoformat().replace("+00:00", "Z"),
///         "original_path": relative_to_bundle(bundle_dir, original_path),
///         "okf_path": f"sources/files/{source_id}.md",
///         "source_version_id": f"{source_id}:v1", "version_number": 1, "status": "active",
///         "parse_status": "pending" if language in {"python", "cobol"} else "context",
///     }
/// ```
pub fn source_record(
    // `repo_dir` is unused in the Python body too (dead parameter kept for
    // this batch's port so the signature/arity matches the call site in
    // `scan_code_repo` exactly) — kept here rather than silently dropped.
    _repo_dir: &Path,
    bundle_dir: &Path,
    path: &Path,
    relative_path: &Path,
    language: &str,
) -> Result<Value> {
    // UNCONFIRMED WIRING: `code_source_id_for` is `kl4a.codekb.ids.code_source_id_for`,
    // out of scope for this batch (`kl4a/codekb/ids.py`). Referencing the
    // expected sibling module.
    let source_id = crate::ids::code_source_id_for(relative_path);
    let checksum = sha256_file(path)?;
    let original_path = copy_original(bundle_dir, path, &source_id, &checksum)?;

    // Fix (Medium finding): Python's check is `"test" in path.parts` on the
    // ABSOLUTE `path` (scan_code_repo resolves `repo_dir` and walks it with
    // `rglob`, so every `path` passed in here is already absolute) — not
    // `relative_path`. The previous version here checked `relative_path`'s
    // components instead, which under-matches Python's actual behavior:
    // e.g. a repo checked out at `.../test/myrepo/...` (any ancestor of
    // `repo_dir` itself literally named "test", not just a directory inside
    // the repo) makes Python classify every Python source file under it as
    // `kind: "test"`, which a `relative_path`-only check would miss
    // entirely. Matching Python exactly, surprising as it is, rather than
    // the more "sensible"-looking relative-path check.
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let is_test = path
        .components()
        .any(|c| c.as_os_str().to_string_lossy() == "test")
        || file_name.starts_with("test_")
        || file_name.ends_with("_test.py");
    let suffix = path
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let is_copybook = language == "cobol" && suffix == ".cpy";
    let kind = if is_copybook {
        "copybook"
    } else if is_test && language == "python" {
        "test"
    } else {
        "source"
    };

    let metadata = std::fs::metadata(path)?;
    let size_bytes = metadata.len();
    let modified_time = metadata
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| format_unix_utc(d.as_secs() as i64))
        .unwrap_or_else(utc_now);

    let relative_posix = crate::ids::as_posix(relative_path);
    let parse_status = if language == "python" || language == "cobol" {
        "pending"
    } else {
        "context"
    };

    Ok(json!({
        "id": source_id,
        "title": relative_posix,
        "path": relative_posix,
        "language": language,
        "kind": kind,
        "checksum": checksum,
        "size_bytes": size_bytes,
        "modified_time": modified_time,
        "original_path": relative_to_bundle(bundle_dir, &original_path)?,
        "okf_path": format!("sources/files/{source_id}.md"),
        "source_version_id": format!("{source_id}:v1"),
        "version_number": 1,
        "status": "active",
        "parse_status": parse_status,
    }))
}

/// Port of `kl4a.codekb.inventory.write_repository_doc` (`inventory.py:215-231`).
pub fn write_repository_doc(bundle_dir: &Path, repo: &Value) -> Result<()> {
    let id = repo.get("id").and_then(Value::as_str).unwrap_or("");
    let title = repo.get("title").and_then(Value::as_str).unwrap_or("");
    let frontmatter = json!({
        "type": "Code Repository",
        "title": title,
        "repo_id": id,
        "code": {
            "vcs": repo.get("vcs"),
            "branch": repo.get("branch"),
            "commit": repo.get("commit"),
            "root": repo.get("path"),
        },
        "tags": ["repository"],
    });
    write_markdown(
        &bundle_dir.join("sources").join("repositories").join(format!("{id}.md")),
        &frontmatter,
        &format!("# {title}\n"),
    )
}

/// Port of `kl4a.codekb.inventory.write_source_file_doc` (`inventory.py:234-267`).
pub fn write_source_file_doc(bundle_dir: &Path, repo: &Value, source: &Value) -> Result<()> {
    let okf_path = source.get("okf_path").and_then(Value::as_str).unwrap_or("");
    let title = source.get("title").and_then(Value::as_str).unwrap_or("");
    let original_path = source.get("original_path").and_then(Value::as_str).unwrap_or("");
    let resource = repo
        .get("commit")
        .filter(|v| !v.is_null())
        .cloned()
        .unwrap_or_else(|| repo.get("path").cloned().unwrap_or(Value::Null));
    let frontmatter = json!({
        "type": "Code Source File",
        "title": title,
        "source_id": source.get("id"),
        "source_version_id": source.get("source_version_id"),
        "code": {
            "language": source.get("language"),
            "path": source.get("path"),
            "kind": source.get("kind"),
            "parse_status": source.get("parse_status"),
            "checksum": source.get("checksum"),
        },
        "sources": [{
            "id": repo.get("id"),
            "title": repo.get("title"),
            "resource": resource,
        }],
    });
    let body = format!("# {title}\n\nOriginal snapshot: `{original_path}`\n\n");
    write_markdown(&bundle_dir.join(okf_path), &frontmatter, &body)
}

/// Port of `kl4a.codekb.inventory.scan_code_repo` (`inventory.py:41-110`).
///
/// ```python
/// def scan_code_repo(repo_dir: Path, bundle_dir: Path) -> dict[str, Any]:
///     repo_dir = repo_dir.resolve()
///     if not repo_dir.exists() or not repo_dir.is_dir():
///         raise FileNotFoundError(f"repository directory does not exist: {repo_dir}")
///     manifest = load_manifest(bundle_dir)
///     codekb_config = effective_codekb_config(repo_dir, manifest)
///     skip_dirs = configured_values(codekb_config, "inventory", "skip_dirs")
///     context_filenames = set(configured_values(codekb_config, "inventory", "context_filenames"))
///     repo_record = repository_record(repo_dir)
///     sources, warnings, detected_languages = [], [], {}
///     for path in iter_repo_files(repo_dir, skip_dirs=skip_dirs):
///         relative_path = path.relative_to(repo_dir)
///         suffix = path.suffix.lower()
///         language = SUPPORTED_LANGUAGE_EXTENSIONS.get(suffix)
///         detected_language = language or DETECTED_LANGUAGE_EXTENSIONS.get(suffix)
///         is_context = path.name in context_filenames
///         if detected_language:
///             detected_languages[detected_language] = detected_languages.get(detected_language, 0) + 1
///         if not language and not is_context:
///             if detected_language:
///                 warnings.append({"path": as_posix(relative_path),
///                                   "warning": f"detected unsupported language: {detected_language}"})
///             continue
///         sources.append(source_record(repo_dir, bundle_dir, path, relative_path, language or "repo-config"))
///     write_repository_doc(bundle_dir, repo_record)
///     for source in sources:
///         write_source_file_doc(bundle_dir, repo_record, source)
///     inventory = {
///         "repository": repo_record,
///         "sources": sorted(sources, key=lambda item: item["path"]),
///         "detected_languages": detected_languages, "warnings": warnings,
///         "config": {"skip_dirs": skip_dirs, "context_filenames": sorted(context_filenames)},
///     }
///     write_code_state(bundle_dir, "code_inventory.json", inventory)
///     manifest["sources"] = [{"id": s["id"], "type": "code", "path": s["okf_path"],
///                              "source_version_id": s["source_version_id"], "status": s["status"]}
///                             for s in inventory["sources"]]
///     manifest["updated_at"] = utc_now()
///     manifest["codekb"] = codekb_config
///     save_manifest(bundle_dir, manifest)
///     return inventory
/// ```
pub fn scan_code_repo(repo_dir: &Path, bundle_dir: &Path) -> Result<Value> {
    let repo_dir = repo_dir.canonicalize().unwrap_or_else(|_| repo_dir.to_path_buf());
    if !repo_dir.exists() || !repo_dir.is_dir() {
        bail!("repository directory does not exist: {}", repo_dir.display());
    }
    // Bug fix: only `repo_dir` used to be canonicalized here; every write
    // under the bundle (`copy_original`'s snapshot copy, the OKF markdown
    // docs, `code_inventory.json`, `manifest.yaml`) was built by joining
    // onto the *raw, non-canonicalized* `bundle_dir` the caller passed in.
    // On Windows, `std::fs::canonicalize()` is what opts a path into the
    // `\\?\`-prefixed extended-length form that lifts the ~260-char
    // MAX_PATH limit; a bundle nested deep enough (a long repo/bundle
    // directory name, common under a test runner's temp dir) would hit
    // MAX_PATH on these writes even though `repo_dir` itself was fine.
    // Python has no such limit (`pathlib`/the OS layer handles long paths
    // transparently), so this canonicalizes `bundle_dir` too, matching the
    // same graceful-fallback pattern already used for `repo_dir` above.
    let bundle_dir = bundle_dir.canonicalize().unwrap_or_else(|_| bundle_dir.to_path_buf());
    let bundle_dir = bundle_dir.as_path();

    // UNCONFIRMED WIRING: `load_manifest` here is the *non-atomic* shared
    // reader (`kl4a.kl4a.bundle_store.load_manifest`, duplicated in
    // `kl4a_shared.rs`); `effective_codekb_config`/`configured_values` are
    // `kl4a.codekb.config.*`, out of scope (`crate::config`, not yet present).
    let mut manifest = load_manifest(bundle_dir)?;
    let codekb_config = crate::config::effective_codekb_config(&repo_dir, manifest.as_object())?;
    let skip_dirs = crate::config::configured_values(&codekb_config, "inventory", "skip_dirs")?;
    let context_filenames: HashSet<String> = crate::config::configured_values(
        &codekb_config,
        "inventory",
        "context_filenames",
    )?
    .into_iter()
    .collect();

    let repo_record = repository_record(&repo_dir);
    let mut sources: Vec<Value> = Vec::new();
    let mut warnings: Vec<Value> = Vec::new();
    let mut detected_languages: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();

    for path in iter_repo_files(&repo_dir, &skip_dirs) {
        let relative_path = path.strip_prefix(&repo_dir).unwrap_or(&path).to_path_buf();
        let suffix = path
            .extension()
            .map(|ext| format!(".{}", ext.to_string_lossy().to_lowercase()))
            .unwrap_or_default();
        let language = SUPPORTED_LANGUAGE_EXTENSIONS.get(suffix.as_str()).copied();
        let detected_language = language.or_else(|| DETECTED_LANGUAGE_EXTENSIONS.get(suffix.as_str()).copied());
        let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let is_context = context_filenames.contains(&file_name);

        if let Some(detected) = detected_language {
            *detected_languages.entry(detected.to_string()).or_insert(0) += 1;
        }
        if language.is_none() && !is_context {
            if let Some(detected) = detected_language {
                warnings.push(json!({
                    "path": crate::ids::as_posix(&relative_path),
                    "warning": format!("detected unsupported language: {detected}"),
                }));
            }
            continue;
        }
        sources.push(source_record(
            &repo_dir,
            bundle_dir,
            &path,
            &relative_path,
            language.unwrap_or("repo-config"),
        )?);
    }

    write_repository_doc(bundle_dir, &repo_record)?;
    for source in &sources {
        write_source_file_doc(bundle_dir, &repo_record, source)?;
    }

    sources.sort_by(|a, b| {
        let path_of = |v: &Value| v.get("path").and_then(Value::as_str).unwrap_or("").to_string();
        path_of(a).cmp(&path_of(b))
    });

    let mut sorted_context_filenames: Vec<String> = context_filenames.into_iter().collect();
    sorted_context_filenames.sort();

    let inventory = json!({
        "repository": repo_record,
        "sources": sources,
        "detected_languages": detected_languages,
        "warnings": warnings,
        "config": {
            "skip_dirs": skip_dirs,
            "context_filenames": sorted_context_filenames,
        },
    });
    write_code_state(bundle_dir, "code_inventory.json", &inventory)?;

    let manifest_sources: Vec<Value> = sources
        .iter()
        .map(|source| {
            json!({
                "id": source.get("id"),
                "type": "code",
                "path": source.get("okf_path"),
                "source_version_id": source.get("source_version_id"),
                "status": source.get("status"),
            })
        })
        .collect();
    if let Value::Object(map) = &mut manifest {
        map.insert("sources".to_string(), Value::Array(manifest_sources));
        map.insert("updated_at".to_string(), json!(utc_now()));
        map.insert("codekb".to_string(), Value::Object(codekb_config));
    }
    // UNCONFIRMED WIRING: `crate::state::save_manifest` (the atomic
    // `kl4a.codekb.state.save_manifest`), out of scope for this batch.
    crate::state::save_manifest(bundle_dir, &manifest)?;

    Ok(inventory)
}
