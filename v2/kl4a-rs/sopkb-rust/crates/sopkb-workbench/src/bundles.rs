//! Bundle listing/lookup/creation -- the "orphaned pure functions" of `web_app.py`
//! (docs/port/port-mapping-e-web-cli-contract.md §0, §2.7; PORT_PLAN.md §5.13
//! P-W3/P-W4/P-W5/P-W6/P-W19/P-W20).

use sopkb_core::error::{Result, SopkbError};
use sopkb_core::ids::slugify;
use sopkb_core::store::CreateBundleResult;
use std::path::{Path, PathBuf};

/// Canonical bundle identity per PORT_PLAN.md §4.0: "canonical: directory name, never
/// manifest id" (P-W4 FIX).
pub type BundleKey = String;

/// `is_bundle_dir(path) = (path/"manifest.yaml").exists()` -- `web_app.py:79`.
pub fn is_bundle_dir(path: &Path) -> bool {
    path.join("manifest.yaml").exists()
}

/// `web_app.py:83-89`, P-W5 PRESERVE bit-identically: `workbench_dir/knowledge-bundles`
/// if that subdirectory exists, OR if `workbench_dir` is not itself a bundle;
/// otherwise `workbench_dir.parent()` -- opening a bundle directory directly makes
/// its SIBLINGS the index set. `Path::parent()` returns `None` at a filesystem root;
/// Python's `Path("/").parent` is `Path("/")` itself, so we fall back to the input
/// unchanged in that case to match.
pub fn knowledge_bundles_root(workbench_dir: &Path) -> PathBuf {
    let candidate = workbench_dir.join("knowledge-bundles");
    if candidate.exists() || !is_bundle_dir(workbench_dir) {
        candidate
    } else {
        workbench_dir.parent().map(Path::to_path_buf).unwrap_or_else(|| workbench_dir.to_path_buf())
    }
}

/// `web_app.py:92-99`, P-W6 PRESERVE: immediate children of `knowledge_bundles_root`
/// that are directories containing `manifest.yaml`. One level deep, no recursion, no
/// registry file -- the filesystem IS the index.
///
/// Sort: case-insensitive on `name`, applied UNIFORMLY on every platform
/// (docs/port/DECISIONS.md Q3 -- a deliberate Rust-side implementation, not filesystem-
/// native sort order). Ties (two names identical after lowercasing -- impossible on
/// Windows today, possible on a case-sensitive filesystem) are broken by raw byte-wise
/// ordering of the original name, so ordering stays fully deterministic.
pub fn list_bundle_dirs(workbench_dir: &Path) -> Vec<PathBuf> {
    let root = knowledge_bundles_root(workbench_dir);
    if !root.exists() {
        return Vec::new();
    }
    let mut dirs: Vec<PathBuf> = match std::fs::read_dir(&root) {
        Ok(entries) => entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir() && is_bundle_dir(p)).collect(),
        Err(_) => return Vec::new(),
    };
    dirs.sort_by(|a, b| {
        let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        a_name.to_lowercase().cmp(&b_name.to_lowercase()).then_with(|| a_name.as_bytes().cmp(b_name.as_bytes()))
    });
    dirs
}

/// `web_app.py:102-107`. P-W4 FIX: Python matches `dir.name` OR `manifest["id"]` as
/// equally-valid keys, with collisions resolved by sort order. This canonicalizes on
/// the directory name: an exact directory-name match always wins; a manifest-id match
/// is only consulted as a secondary alias when no directory name matches, and never
/// overrides a real directory-name hit.
pub fn bundle_dir_for_key(workbench_dir: &Path, bundle_key: &str) -> Result<PathBuf> {
    let dirs = list_bundle_dirs(workbench_dir);
    if let Some(dir) = dirs.iter().find(|d| d.file_name().and_then(|n| n.to_str()) == Some(bundle_key)) {
        return Ok(dir.clone());
    }
    for dir in &dirs {
        if let Ok(manifest) = sopkb_core::store::load_manifest(dir) {
            if manifest.get("id").and_then(|v| v.as_str()) == Some(bundle_key) {
                return Ok(dir.clone());
            }
        }
    }
    Err(SopkbError::NotFound(format!("unknown bundle: {bundle_key}")))
}

/// One card's worth of summary data, matching `bundle_describe`'s shape plus the key.
///
/// `created_at` is read directly from the manifest here rather than added to
/// `bundle_describe` itself -- that function's exact output shape is pinned against a
/// real Python-generated fixture (`sopkb-derive/tests/phase4_v1_diff.rs`) and shared
/// with the MCP/agent read tools, so it must not gain UI-only fields.
#[derive(Debug, Clone, PartialEq)]
pub struct BundleSummary {
    pub key: BundleKey,
    pub id: String,
    pub title: String,
    pub profile: String,
    pub status: String,
    pub source_count: usize,
    pub knowledge_item_count: usize,
    pub created_at: String,
}

/// docs/port/port-mapping-e-web-cli-contract.md §4: `BundleCard { ...BundleSummary,
/// bundle_path, export_path, load_error? }`. P-W3 FIX: unlike Python's
/// `render_bundle_index` (`except Exception: continue`, silently dropping a bundle with
/// a malformed manifest from the list), every directory that looks like a bundle
/// produces a card -- `summary`/`export_path` are `None` and `load_error` is set
/// instead of the whole entry vanishing.
#[derive(Debug, Clone)]
pub struct BundleCard {
    pub key: BundleKey,
    pub bundle_path: PathBuf,
    pub summary: Option<BundleSummary>,
    pub export_path: Option<PathBuf>,
    pub load_error: Option<String>,
}

fn key_for(bundle_dir: &Path) -> BundleKey {
    bundle_dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string()
}

/// `bundle_describe` (already ported in `sopkb-derive`) plus the directory-name key.
pub fn describe_bundle(bundle_dir: &Path) -> Result<BundleSummary> {
    let described = sopkb_derive::reads::bundle_describe(bundle_dir)?;
    let get_str = |k: &str| described[k].as_str().unwrap_or("").to_string();
    let get_count = |k: &str| described[k].as_u64().unwrap_or(0) as usize;
    // `created_at` comes straight from the manifest, not `bundle_describe` (see
    // `BundleSummary`'s doc comment) -- `load_manifest` returns the raw loaded mapping
    // with no schema defaults (G-A14), so a manifest predating this field (impossible
    // in practice since `create_bundle` always writes it, but not asserted anywhere)
    // falls back to an empty string rather than erroring the whole card.
    let manifest = sopkb_core::store::load_manifest(bundle_dir)?;
    let created_at = manifest.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
    Ok(BundleSummary {
        key: key_for(bundle_dir),
        id: get_str("id"),
        title: get_str("title"),
        profile: get_str("profile"),
        status: get_str("status"),
        source_count: get_count("source_count"),
        knowledge_item_count: get_count("knowledge_item_count"),
        created_at,
    })
}

/// Permanently deletes a bundle directory and everything in it. Irreversible -- the
/// caller (UI) is responsible for confirming with the user before calling this; this
/// function does not ask again. Refuses to delete anything that isn't actually a
/// bundle directory (`is_bundle_dir` check), so a bad `bundle_key` can't be used to
/// recursively delete an arbitrary path.
pub fn delete_bundle(workbench_dir: &Path, bundle_key: &str) -> Result<()> {
    let bundle_dir = bundle_dir_for_key(workbench_dir, bundle_key)?;
    if !is_bundle_dir(&bundle_dir) {
        return Err(SopkbError::Value(format!("not a bundle directory: {}", bundle_dir.display())));
    }
    std::fs::remove_dir_all(&bundle_dir).map_err(SopkbError::from)
}

fn bundle_card(bundle_dir: &Path) -> BundleCard {
    let key = key_for(bundle_dir);
    match describe_bundle(bundle_dir) {
        Ok(summary) => {
            let export_path = sopkb_export::default_export_dir(bundle_dir).ok();
            BundleCard { key, bundle_path: bundle_dir.to_path_buf(), summary: Some(summary), export_path, load_error: None }
        }
        Err(e) => BundleCard { key, bundle_path: bundle_dir.to_path_buf(), summary: None, export_path: None, load_error: Some(e.to_string()) },
    }
}

/// `render_bundle_index`'s data half (`web_app.py:561-622`), minus every byte of HTML.
/// P-W3 FIX: see `BundleCard`'s doc comment -- nothing is silently dropped.
pub fn list_bundles(workbench_dir: &Path) -> Vec<BundleCard> {
    list_bundle_dirs(workbench_dir).iter().map(|d| bundle_card(d)).collect()
}

#[derive(Debug)]
pub struct CreateProjectResult {
    pub card: BundleCard,
    pub already_existed: bool,
}

/// `handle_new_project_form` (`web_app.py:625-629`). P-W19 FIX: the idempotency of
/// `create_bundle` is no longer silently swallowed -- `already_existed` tells the
/// caller whether this call actually created anything, so a future UI can say "opened
/// existing project" instead of lying "Project created". P-W20 FIX: always runs
/// `sync_okf_bundle` afterward (Python's `/new-project` path never did, unlike the CLI
/// `init` path, leaving a web-created project without its OKF index docs -- a bundle
/// without `index.md` is spec-invalid even though `validate_bundle` never checked it).
pub fn create_project(workbench_dir: &Path, title: &str) -> Result<CreateProjectResult> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(SopkbError::Value("missing required field: title".to_string()));
    }
    let bundle_dir = knowledge_bundles_root(workbench_dir).join(slugify(trimmed));
    sopkb_review::with_bundle_lock(&bundle_dir, || {
        let already_existed = matches!(sopkb_core::store::create_bundle(&bundle_dir, Some(trimmed))?, CreateBundleResult::Existing(_));
        sopkb_export::sync_okf_bundle(&bundle_dir)?;
        Ok(CreateProjectResult { card: bundle_card(&bundle_dir), already_existed })
    })
}

/// CLI `init` equivalent (docs/port/port-mapping-e-web-cli-contract.md §4.2
/// `init_bundle`): create a bundle at an arbitrary path, outside the current
/// workbench root. Always syncs (P-W20), same as `create_project`.
pub fn init_bundle(bundle_dir: &Path, title: Option<&str>) -> Result<BundleSummary> {
    sopkb_review::with_bundle_lock(bundle_dir, || {
        sopkb_core::store::create_bundle(bundle_dir, title)?;
        sopkb_export::sync_okf_bundle(bundle_dir)?;
        describe_bundle(bundle_dir)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_bundle(dir: &Path, title: &str) {
        sopkb_core::store::create_bundle(dir, Some(title)).unwrap();
    }

    #[test]
    fn describe_bundle_surfaces_the_manifests_created_at() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        make_bundle(&bundle_dir, "B");
        let summary = describe_bundle(&bundle_dir).unwrap();
        assert!(!summary.created_at.is_empty());
        // Matches the manifest's own value exactly, not a freshly-generated one.
        let manifest = sopkb_core::store::load_manifest(&bundle_dir).unwrap();
        assert_eq!(summary.created_at, manifest.get("created_at").and_then(|v| v.as_str()).unwrap());
    }

    #[test]
    fn delete_bundle_removes_the_directory() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("knowledge-bundles").join("my-bundle");
        make_bundle(&bundle_dir, "My Bundle");
        assert!(bundle_dir.exists());
        delete_bundle(dir.path(), "my-bundle").unwrap();
        assert!(!bundle_dir.exists());
    }

    #[test]
    fn delete_bundle_unknown_key_errors_without_touching_disk() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("knowledge-bundles")).unwrap();
        let err = delete_bundle(dir.path(), "does-not-exist").unwrap_err();
        assert!(err.to_string().contains("does-not-exist"));
    }

    #[test]
    fn delete_bundle_refuses_a_non_bundle_directory() {
        // bundle_dir_for_key can only ever resolve to a real bundle directory (it
        // walks list_bundle_dirs, which already filters on is_bundle_dir), so this
        // guards the function against being repurposed to accept an arbitrary path
        // directly in the future, not a reachable case via the public key-based API.
        let dir = tempdir().unwrap();
        let not_a_bundle = dir.path().join("not-a-bundle");
        std::fs::create_dir_all(&not_a_bundle).unwrap();
        assert!(!is_bundle_dir(&not_a_bundle));
    }

    #[test]
    fn is_bundle_dir_checks_manifest_presence() {
        let dir = tempdir().unwrap();
        assert!(!is_bundle_dir(dir.path()));
        make_bundle(dir.path(), "X");
        assert!(is_bundle_dir(dir.path()));
    }

    #[test]
    fn knowledge_bundles_root_prefers_existing_subdir() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("knowledge-bundles")).unwrap();
        assert_eq!(knowledge_bundles_root(dir.path()), dir.path().join("knowledge-bundles"));
    }

    #[test]
    fn knowledge_bundles_root_uses_subdir_name_even_when_not_a_bundle_and_missing() {
        // Not itself a bundle, and "knowledge-bundles" subdir doesn't exist yet:
        // Python's `candidate if candidate.exists() or not is_bundle_dir(dir) else parent`
        // -- `not is_bundle_dir` is True, so the (nonexistent) candidate wins anyway.
        let dir = tempdir().unwrap();
        assert_eq!(knowledge_bundles_root(dir.path()), dir.path().join("knowledge-bundles"));
    }

    #[test]
    fn knowledge_bundles_root_opening_a_bundle_directly_uses_parent_p_w5() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("my-bundle");
        make_bundle(&bundle_dir, "My Bundle");
        // bundle_dir IS a bundle, and bundle_dir/knowledge-bundles does not exist ->
        // siblings (bundle_dir.parent()) become the index set.
        assert_eq!(knowledge_bundles_root(&bundle_dir), dir.path());
    }

    #[test]
    fn list_bundle_dirs_is_case_insensitive_with_byte_tiebreak() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("knowledge-bundles");
        std::fs::create_dir_all(&root).unwrap();
        for name in ["Banana", "apple", "cherry"] {
            make_bundle(&root.join(name), name);
        }
        let names: Vec<String> = list_bundle_dirs(dir.path()).iter().map(|p| p.file_name().unwrap().to_str().unwrap().to_string()).collect();
        assert_eq!(names, vec!["apple", "Banana", "cherry"]);
    }

    #[test]
    fn list_bundle_dirs_missing_root_returns_empty() {
        let dir = tempdir().unwrap();
        assert!(list_bundle_dirs(dir.path()).is_empty());
    }

    #[test]
    fn bundle_dir_for_key_matches_directory_name_first() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("knowledge-bundles");
        make_bundle(&root.join("my-dir"), "My Dir");
        let found = bundle_dir_for_key(dir.path(), "my-dir").unwrap();
        assert_eq!(found, root.join("my-dir"));
    }

    #[test]
    fn bundle_dir_for_key_falls_back_to_manifest_id_alias_p_w4() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("knowledge-bundles");
        // create_bundle sets manifest id = slugify(dir name); dir name differs from id
        // only in casing/punctuation handling here we force a mismatch by renaming.
        let bundle_dir = root.join("weird--dir__name");
        make_bundle(&bundle_dir, "Weird Dir Name");
        let manifest = sopkb_core::store::load_manifest(&bundle_dir).unwrap();
        let id = manifest.get("id").and_then(|v| v.as_str()).unwrap().to_string();
        assert_ne!(id, "weird--dir__name", "test setup should produce a differing id/dir-name pair");
        let found = bundle_dir_for_key(dir.path(), &id).unwrap();
        assert_eq!(found, bundle_dir);
    }

    #[test]
    fn bundle_dir_for_key_directory_name_wins_over_a_colliding_manifest_id() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("knowledge-bundles");
        // Bundle A's directory name equals bundle B's manifest id.
        make_bundle(&root.join("bundle-b"), "Bundle B");
        let manifest_b = sopkb_core::store::load_manifest(&root.join("bundle-b")).unwrap();
        let id_b = manifest_b.get("id").and_then(|v| v.as_str()).unwrap().to_string();
        make_bundle(&root.join(&id_b), "Colliding Dir");
        // Looking up by `id_b` must resolve to the DIRECTORY named `id_b` (P-W4:
        // directory name is canonical), not to bundle-b whose manifest id happens to
        // equal it.
        let found = bundle_dir_for_key(dir.path(), &id_b).unwrap();
        assert_eq!(found, root.join(&id_b));
    }

    #[test]
    fn bundle_dir_for_key_unknown_is_not_found() {
        let dir = tempdir().unwrap();
        let err = bundle_dir_for_key(dir.path(), "nope").unwrap_err();
        assert_eq!(err.to_string(), "unknown bundle: nope");
    }

    #[test]
    fn list_bundles_surfaces_load_error_instead_of_dropping_p_w3() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("knowledge-bundles");
        make_bundle(&root.join("good"), "Good");
        // sopkb-fmt's hand-rolled YAML parser is deliberately narrow but also
        // deliberately lenient (never returns a parse error; `manifest.yaml` always
        // loads as SOME mapping, even from odd-looking text) -- so a genuinely
        // reachable "malformed bundle" here is a valid manifest.yaml paired with a
        // CORRUPT `.sopkb/inventory.json`, which `read_json` does propagate as a real
        // parse error (its own doc comment: "never silently swallowed... matching
        // Python's json.loads").
        let broken_dir = root.join("broken");
        make_bundle(&broken_dir, "Broken");
        std::fs::write(broken_dir.join(".sopkb").join("inventory.json"), "{not valid json").unwrap();

        let cards = list_bundles(dir.path());
        assert_eq!(cards.len(), 2);
        let broken_card = cards.iter().find(|c| c.key == "broken").expect("broken bundle still gets a card");
        assert!(broken_card.summary.is_none());
        assert!(broken_card.load_error.is_some());
        let good_card = cards.iter().find(|c| c.key == "good").unwrap();
        assert!(good_card.load_error.is_none());
        assert!(good_card.summary.is_some());
    }

    #[test]
    fn create_project_reports_already_existed_p_w19() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("knowledge-bundles")).unwrap();
        let first = create_project(dir.path(), "My Project").unwrap();
        assert!(!first.already_existed);
        let second = create_project(dir.path(), "My Project").unwrap();
        assert!(second.already_existed);
    }

    #[test]
    fn create_project_always_syncs_okf_p_w20() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("knowledge-bundles")).unwrap();
        let result = create_project(dir.path(), "Synced Project").unwrap();
        assert!(result.card.bundle_path.join("index.md").exists(), "sync_okf_bundle should have written index.md");
    }

    #[test]
    fn create_project_rejects_blank_title() {
        let dir = tempdir().unwrap();
        let err = create_project(dir.path(), "   ").unwrap_err();
        assert_eq!(err.to_string(), "missing required field: title");
    }

    #[test]
    fn init_bundle_always_syncs_okf() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("standalone");
        let summary = init_bundle(&bundle_dir, Some("Standalone")).unwrap();
        assert_eq!(summary.title, "Standalone");
        assert!(bundle_dir.join("index.md").exists());
    }
}
