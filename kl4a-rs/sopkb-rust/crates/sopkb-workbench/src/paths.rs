//! Shared path-resolution helper. Both `context::resolve_launch_target`'s
//! `expanduser().resolve()` step and `upload::stage_uploaded_files`'s traversal guard
//! need the same thing Python's `Path.resolve(strict=False)` gives them: an absolute,
//! symlink-resolved path that works even when the target (or a trailing part of it)
//! doesn't exist yet on disk.
//!
//! `std::fs::canonicalize` can't be used directly for this: it requires the full path
//! to exist, and on Windows it prepends the `\\?\` extended-length prefix, which would
//! make every path this crate returns look different from what the user typed or what
//! a native file dialog reported (P-W25 is explicitly out of scope for this phase, but
//! there is no reason to introduce that noise gratuitously).

use std::path::{Component, Path, PathBuf};

/// Lexically absolute-izes and normalizes (`.`/`..`) `path` with NO filesystem access
/// at all -- no symlink resolution, no existence check. Used as the first step of
/// [`resolve_best_effort`], and exposed on its own for call sites that only need
/// "make this comparable/joinable" without caring about symlinks.
pub fn normalize_lexical(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
    };
    let mut out = PathBuf::new();
    for comp in absolute.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Best-effort equivalent of Python's `Path.resolve(strict=False)` (the default,
/// non-strict form Python uses throughout `web_app.py`/`desktop.py`): resolves
/// symlinks for as much of `path` as exists on disk, then lexically appends whatever
/// tail doesn't exist yet. Never requires the full path to exist (unlike
/// `std::fs::canonicalize`) and never grows a `\\?\` prefix unless the OS itself
/// already uses one for the resolved existing ancestor.
pub fn resolve_best_effort(path: &Path) -> PathBuf {
    let absolute = normalize_lexical(path);

    let mut ancestor: &Path = &absolute;
    let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
    loop {
        if ancestor.exists() {
            break;
        }
        match (ancestor.file_name(), ancestor.parent()) {
            (Some(name), Some(parent)) => {
                tail.push(name);
                ancestor = parent;
            }
            _ => break, // reached a root with nothing existing -- give up resolving further
        }
    }

    let mut resolved = std::fs::canonicalize(ancestor).map(strip_windows_verbatim_prefix).unwrap_or_else(|_| ancestor.to_path_buf());
    for name in tail.into_iter().rev() {
        resolved.push(name);
    }
    resolved
}

/// `std::fs::canonicalize` on Windows always returns the `\\?\`-prefixed
/// extended-length form (and `\\?\UNC\server\share` for UNC paths). Strip it back to
/// the ordinary form callers actually typed/expect, matching what Python's
/// `Path.resolve()` returns (no such prefix). A no-op on non-Windows platforms, where
/// `canonicalize` never produces this prefix in the first place.
fn strip_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    match path.to_str() {
        Some(s) => {
            if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
                PathBuf::from(format!(r"\\{rest}"))
            } else if let Some(rest) = s.strip_prefix(r"\\?\") {
                PathBuf::from(rest)
            } else {
                path
            }
        }
        None => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_existing_path_via_real_canonicalize() {
        let dir = tempfile::tempdir().unwrap();
        let resolved = resolve_best_effort(dir.path());
        assert!(resolved.exists());
        assert!(resolved.is_absolute());
    }

    #[test]
    fn resolves_nonexistent_tail_lexically_under_existing_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("does").join("not").join("exist.txt");
        let resolved = resolve_best_effort(&target);
        // The existing ancestor (dir.path()) portion should be canonicalized; the
        // nonexistent tail should be appended verbatim.
        assert_eq!(resolved.file_name().unwrap(), "exist.txt");
        assert!(resolved.parent().unwrap().ends_with(Path::new("does").join("not")));
    }

    #[test]
    fn normalize_lexical_collapses_dot_and_dotdot() {
        let dir = tempfile::tempdir().unwrap();
        let messy = dir.path().join("a").join(".").join("..").join("b");
        let normalized = normalize_lexical(&messy);
        assert_eq!(normalized, dir.path().join("b"));
    }
}
