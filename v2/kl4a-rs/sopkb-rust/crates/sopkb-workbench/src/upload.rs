//! Upload staging. docs/port/port-mapping-e-web-cli-contract.md §2.2 (`web_app.py:357-443`);
//! docs/port/DECISIONS.md Q8; PORT_PLAN.md P-W13, P-W14, P-W15, P-W16, P-W26.
//!
//! P-W16 FIX: no bytes cross this crate's API at all -- Python's `UploadedSourceFile`
//! carries `content: bytes` because HTTP multipart delivers bytes; the Tauri-free
//! equivalent here takes a list of already-resolved SOURCE FILE PATHS (from a native
//! file dialog, or from any other caller) and reads/copies them itself. There is no
//! multipart form to parse in this port at all.
//!
//! Per docs/port/DECISIONS.md Q8, folder picks bypass staging entirely (point
//! `scan_sources` straight at the picked directory -- see `crate::ingest::IngestSource`);
//! only flat/multi-file picks route through this module.

use crate::paths::resolve_best_effort;
use sopkb_core::error::{Result, SopkbError};
use sopkb_core::store::state_path;
use std::path::{Path, PathBuf};

/// One already-picked source file to stage. `relative_name` mirrors Python's
/// `UploadedSourceFile.filename` -- the original filename OR relative sub-path (a
/// folder-shaped multi-file pick can still arrive here with `relative_name` like
/// `"policies/simple_upload.md"`, preserved through staging), never the file's actual
/// on-disk absolute path.
#[derive(Debug, Clone)]
pub struct UploadSource {
    pub path: PathBuf,
    pub relative_name: String,
}

#[derive(Debug, Clone)]
pub struct StagedUpload {
    pub staging_dir: PathBuf,
    pub file_count: usize,
}

/// `sanitize_upload_segment` (`web_app.py:440-443`): `re.sub(r"[^A-Za-z0-9._ -]+", "_",
/// segment.strip())`, then strip leading/trailing spaces and dots from the result,
/// falling back to the literal `"upload"` if that leaves nothing.
pub fn sanitize_upload_segment(segment: &str) -> String {
    let trimmed = segment.trim();
    let mut cleaned = String::with_capacity(trimmed.len());
    let mut in_run = false;
    for ch in trimmed.chars() {
        let allowed = ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | ' ' | '-');
        if allowed {
            cleaned.push(ch);
            in_run = false;
        } else if !in_run {
            cleaned.push('_');
            in_run = true;
        }
    }
    let stripped = cleaned.trim_matches(|c| c == ' ' || c == '.');
    if stripped.is_empty() {
        "upload".to_string()
    } else {
        stripped.to_string()
    }
}

/// `safe_upload_relative_path` (`web_app.py:428-437`): split on runs of `\`/`/`, drop
/// empty/`.`/`..` segments, sanitize each remaining segment, join. Empty result ->
/// `"uploaded file is missing a filename"` (P-W15 PRESERVE-adjacent: this specific
/// error, not the zero-upload one).
pub fn safe_upload_relative_path(filename: &str) -> Result<PathBuf> {
    let parts: Vec<String> = filename
        .split(['\\', '/'])
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .map(sanitize_upload_segment)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return Err(SopkbError::Value("uploaded file is missing a filename".to_string()));
    }
    Ok(parts.iter().collect())
}

/// `reset_upload_directory` (`web_app.py:422-425`): rmtree + mkdir.
pub fn reset_upload_directory(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    std::fs::create_dir_all(path)?;
    Ok(())
}

/// `stage_uploaded_files` (`web_app.py:398-419`), with docs/port/DECISIONS.md Q8's
/// `replace: bool` added (default-`true` behavior preserves today's wipe-every-time
/// semantics via [`reset_upload_directory`]; `replace: false` skips the wipe so files
/// accumulate across calls -- "add more files" semantics).
///
/// P-W26 FIX: `state_path` itself performs no sanitization on the trailing component,
/// so the only user-influenced segments here (each upload's `relative_name`) are
/// routed through [`safe_upload_relative_path`] AND the belt-and-braces traversal
/// check below (P-W14 PRESERVE) before ever touching disk -- a crafted `relative_name`
/// (or a symlink planted inside a folder pick) cannot escape `.sopkb/uploads/current`.
pub fn stage_uploaded_files(bundle_dir: &Path, uploads: &[UploadSource], replace: bool) -> Result<StagedUpload> {
    sopkb_review::with_bundle_lock(bundle_dir, || stage_uploaded_files_locked(bundle_dir, uploads, replace))
}

fn stage_uploaded_files_locked(bundle_dir: &Path, uploads: &[UploadSource], replace: bool) -> Result<StagedUpload> {
    let staging_dir = state_path(bundle_dir, "uploads").join("current");
    if replace {
        reset_upload_directory(&staging_dir)?;
    } else {
        std::fs::create_dir_all(&staging_dir)?;
    }
    let resolved_staging = resolve_best_effort(&staging_dir);

    let mut file_count = 0usize;
    for upload in uploads {
        let relative_path = safe_upload_relative_path(&upload.relative_name)?;
        let destination = staging_dir.join(&relative_path);

        // Belt-and-braces traversal check BEFORE creating any directories (P-W14
        // PRESERVE), matching Python's own ordering: resolve first (best-effort,
        // against whatever already exists), only mkdir/write if that resolves safely.
        let resolved_destination = resolve_best_effort(&destination);
        if !resolved_destination.starts_with(&resolved_staging) {
            return Err(SopkbError::Value(format!("unsafe upload path: {}", upload.relative_name)));
        }

        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&upload.path, &destination)?;
        file_count += 1;
    }

    if file_count == 0 {
        return Err(SopkbError::Value("no files were selected".to_string()));
    }
    Ok(StagedUpload { staging_dir, file_count })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sanitize_upload_segment_collapses_bad_runs_and_strips_dots_spaces() {
        assert_eq!(sanitize_upload_segment("hello world.md"), "hello world.md");
        assert_eq!(sanitize_upload_segment("weird***name???.md"), "weird_name_.md");
        assert_eq!(sanitize_upload_segment("  .leading.trailing.  "), "leading.trailing");
        // A run of disallowed characters collapses to a SINGLE "_", which is itself a
        // non-empty, non-dot/space result -- so it is NOT the empty string that would
        // trigger the "upload" fallback (`"_" or "upload"` in Python is `"_"`, since
        // `"_"` is truthy).
        assert_eq!(sanitize_upload_segment("***"), "_");
        // Only when the sanitized-then-stripped result is genuinely EMPTY does the
        // fallback kick in: an all-dots-and-spaces segment survives the regex (`.` and
        // ` ` are both in the allowed set) but is then stripped away entirely by
        // `.strip(" .")`.
        assert_eq!(sanitize_upload_segment("..."), "upload");
        assert_eq!(sanitize_upload_segment(""), "upload");
    }

    #[test]
    fn safe_upload_relative_path_preserves_nested_subpaths() {
        let path = safe_upload_relative_path("policies/simple_upload.md").unwrap();
        assert_eq!(path, PathBuf::from("policies").join("simple_upload.md"));
    }

    #[test]
    fn safe_upload_relative_path_drops_dot_and_dotdot_segments() {
        let path = safe_upload_relative_path("../../etc/passwd").unwrap();
        assert_eq!(path, PathBuf::from("etc").join("passwd"));
    }

    #[test]
    fn safe_upload_relative_path_handles_backslash_and_forward_slash() {
        let path = safe_upload_relative_path("a\\b/c.txt").unwrap();
        assert_eq!(path, PathBuf::from("a").join("b").join("c.txt"));
    }

    #[test]
    fn safe_upload_relative_path_empty_result_is_an_error() {
        let err = safe_upload_relative_path("../..").unwrap_err();
        assert_eq!(err.to_string(), "uploaded file is missing a filename");
        let err = safe_upload_relative_path("").unwrap_err();
        assert_eq!(err.to_string(), "uploaded file is missing a filename");
    }

    fn make_bundle(dir: &Path) -> PathBuf {
        let bundle_dir = dir.join("bundle");
        sopkb_core::store::create_bundle(&bundle_dir, Some("B")).unwrap();
        bundle_dir
    }

    fn source_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn stage_uploaded_files_preserves_relative_subpaths_on_disk() {
        let dir = tempdir().unwrap();
        let bundle_dir = make_bundle(dir.path());
        let src = source_file(dir.path(), "upload.md", b"# Eligibility\n");
        let uploads = vec![UploadSource { path: src, relative_name: "policies/simple_upload.md".to_string() }];

        let staged = stage_uploaded_files(&bundle_dir, &uploads, true).unwrap();
        assert_eq!(staged.file_count, 1);
        assert!(bundle_dir.join(".sopkb").join("uploads").join("current").join("policies").join("simple_upload.md").exists());
    }

    #[test]
    fn stage_uploaded_files_zero_files_is_an_error_p_w15() {
        let dir = tempdir().unwrap();
        let bundle_dir = make_bundle(dir.path());
        let err = stage_uploaded_files(&bundle_dir, &[], true).unwrap_err();
        assert_eq!(err.to_string(), "no files were selected");
    }

    #[test]
    fn stage_uploaded_files_replace_true_wipes_previous_set() {
        let dir = tempdir().unwrap();
        let bundle_dir = make_bundle(dir.path());
        let first = source_file(dir.path(), "first.md", b"first");
        stage_uploaded_files(&bundle_dir, &[UploadSource { path: first, relative_name: "first.md".to_string() }], true).unwrap();

        let second = source_file(dir.path(), "second.md", b"second");
        let staged = stage_uploaded_files(&bundle_dir, &[UploadSource { path: second, relative_name: "second.md".to_string() }], true).unwrap();

        assert_eq!(staged.file_count, 1);
        assert!(!staged.staging_dir.join("first.md").exists(), "replace:true must wipe the previous set");
        assert!(staged.staging_dir.join("second.md").exists());
    }

    #[test]
    fn stage_uploaded_files_replace_false_accumulates_q8() {
        let dir = tempdir().unwrap();
        let bundle_dir = make_bundle(dir.path());
        let first = source_file(dir.path(), "first.md", b"first");
        stage_uploaded_files(&bundle_dir, &[UploadSource { path: first, relative_name: "first.md".to_string() }], true).unwrap();

        let second = source_file(dir.path(), "second.md", b"second");
        let staged = stage_uploaded_files(&bundle_dir, &[UploadSource { path: second, relative_name: "second.md".to_string() }], false).unwrap();

        assert_eq!(staged.file_count, 1);
        assert!(staged.staging_dir.join("first.md").exists(), "replace:false must keep the previous set");
        assert!(staged.staging_dir.join("second.md").exists());
    }

    #[cfg(unix)]
    #[test]
    fn stage_uploaded_files_rejects_symlink_escape_p_w14() {
        let dir = tempdir().unwrap();
        let bundle_dir = make_bundle(dir.path());
        let staging_dir = state_path(&bundle_dir, "uploads").join("current");
        std::fs::create_dir_all(&staging_dir).unwrap();
        let outside = tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), staging_dir.join("escape")).unwrap();

        let src = source_file(dir.path(), "evil.md", b"evil");
        let uploads = vec![UploadSource { path: src, relative_name: "escape/evil.md".to_string() }];
        let err = stage_uploaded_files(&bundle_dir, &uploads, false).unwrap_err();
        assert!(err.to_string().starts_with("unsafe upload path:"));
    }
}
