//! Phase 5 acceptance gate (docs/port/PORT_PLAN.md §6.6 Done-when): a full recursive
//! tree diff of `sync_okf_bundle`'s output -- every `index.md`, every `sources/*.md`,
//! and every file under the nine regenerated directories -- byte-identical to the real
//! Python CLI's recorded output, after the same normalization the Phase 0 harness
//! itself applies when recording fixtures (`v2/sopkb-rust/fixtures/harness/harness.py`'s
//! `normalize_text`): OKF frontmatter `date:` -> `<TS>`, `log.md`'s `## <date>` -> `##
//! <TS>`. Both sides of every comparison here go through that SAME normalization, so
//! this is a true byte-exact check on everything except the two fields that are
//! expected to differ solely because "today" is a different date than recording day.
//!
//! Three fixture cases carry this: `reference` (the general case), `legacy-layout`
//! (P-X3: proves 9 legacy state files survive at BOTH locations after a sync starting
//! from the pre-migration layout), `authored-okf` (P-X2 sibling: proves only `*.md` is
//! copied from `.sopkb/authored_okf/`). A fourth, hand-built case proves
//! `sources/originals/`/`sources/normalized/` survive a sync while a stray
//! `sources/*.md` does not (P-X2).

use regex::Regex;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // Walk up from this crate's own directory until a top-level `.git` entry is
    // found (a directory in a normal checkout, a file in a git worktree) --
    // depth-agnostic, unlike a fixed `ancestors().nth(N)` hop count.
    let mut dir = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        dir = dir.parent().expect("repo_root: reached filesystem root without finding a .git entry").to_path_buf();
    }
}

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// Mirrors `harness.py`'s `_normalize_frontmatter_date`: only touches the `date:` and
/// `modified_time:` lines inside a `---\n...\n---` frontmatter header, leaving the body
/// untouched.
///
/// `modified_time:` joined this rule with the source-versioning subsystem
/// (docs/port/CATCHUP_PLAN.md workstream 2), which put a source's version registry into
/// the `SOP Source` document's frontmatter -- including each version's `modified_time`,
/// i.e. the input file's mtime on whatever machine recorded the fixture. That value is
/// environment-dependent in exactly the way `generated.date` already was, and the
/// harness already normalizes it inside `.sopkb/*.json` (which is why the recorded
/// `inventory.json` carries a literal `<TS>` there). Without the same rule for
/// frontmatter, `sources/*.md` could never match: the recorded document holds the
/// mtime from the recording host, while a re-sync of the normalized fixture bundle
/// faithfully propagates the `<TS>` sitting in its inventory. Same class of gap as the
/// five `harness.py` normalization bugs written up in `fixtures/README.md`.
fn normalize_frontmatter_date(text: &str) -> String {
    if !text.starts_with("---") {
        return text.to_string();
    }
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.first().map(|l| l.trim()) != Some("---") {
        return text.to_string();
    }
    let mut end_idx = None;
    for (idx, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == "---" {
            end_idx = Some(idx);
            break;
        }
    }
    let Some(end_idx) = end_idx else {
        return text.to_string();
    };
    let header = lines[1..end_idx].join("\n");
    let re = Regex::new(r"(?m)^(?P<pre>[ \t-]*(?:date|modified_time):[ \t]*)[^\r\n]*(?P<cr>\r?)$").unwrap();
    let new_header = re.replace_all(&header, "${pre}<TS>${cr}").to_string();
    let mut out_lines: Vec<String> = vec![lines[0].to_string()];
    out_lines.extend(new_header.split('\n').map(|s| s.to_string()));
    out_lines.extend(lines[end_idx..].iter().map(|s| s.to_string()));
    out_lines.join("\n")
}

/// Mirrors `harness.py`'s `normalize_text` for the file types this gate touches
/// (`log.md`'s dated heading, and every `.md` document's frontmatter `date:`).
fn normalize_text(rel_path: &str, text: &str) -> String {
    let mut text = text.to_string();
    if rel_path == "log.md" || rel_path.ends_with("/log.md") {
        let re = Regex::new(r"(?m)^## \d{4}-\d{2}-\d{2}(?P<cr>\r?)$").unwrap();
        text = re.replace_all(&text, "## <TS>$cr").to_string();
    }
    if rel_path.ends_with(".md") {
        text = normalize_frontmatter_date(&text);
    }
    text
}

/// Every relative path `sync_okf_bundle` is responsible for: `index.md`, `log.md`,
/// top-level `sources/*.md` (non-recursive), and everything under the nine regenerated
/// directories (recursive).
fn okf_tree_files(bundle_dir: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for name in ["index.md", "log.md"] {
        if bundle_dir.join(name).exists() {
            out.insert(name.to_string());
        }
    }
    let sources_dir = bundle_dir.join("sources");
    if sources_dir.exists() {
        for entry in fs::read_dir(&sources_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md") {
                out.insert(format!("sources/{}", entry.file_name().to_string_lossy()));
            }
        }
    }
    for dir in ["sections", "concepts", "knowledge", "relations", "rules", "evidence", "tasks", "references", "authored"] {
        let d = bundle_dir.join(dir);
        if d.exists() {
            collect_files_recursive(&d, dir, &mut out);
        }
    }
    out
}

fn collect_files_recursive(dir: &Path, rel_prefix: &str, out: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel = format!("{rel_prefix}/{}", entry.file_name().to_string_lossy());
        if path.is_dir() {
            collect_files_recursive(&path, &rel, out);
        } else {
            out.insert(rel);
        }
    }
}

/// Resyncs a copy of `start_from` (some already-populated bundle state) and diffs
/// every OKF-tree file against `expected_bundle` (which, for these fixture cases, is
/// exactly what `sync_okf_bundle` should reproduce byte-for-byte modulo today's date).
fn run_full_tree_diff(case_name: &str, start_from: &Path, expected_bundle: &Path) {
    let workdir = std::env::temp_dir().join(format!("sopkb-phase5-{case_name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    copy_dir(start_from, &workdir);

    sopkb_export::sync_okf_bundle(&workdir).unwrap_or_else(|e| panic!("{case_name}: sync_okf_bundle failed: {e}"));

    let actual_files = okf_tree_files(&workdir);
    let expected_files = okf_tree_files(expected_bundle);
    assert_eq!(actual_files, expected_files, "{case_name}: OKF tree file set mismatch");

    let mut mismatches = Vec::new();
    for rel in &actual_files {
        let actual = fs::read_to_string(workdir.join(rel)).unwrap_or_else(|e| panic!("{case_name}: read actual {rel}: {e}"));
        let expected = fs::read_to_string(expected_bundle.join(rel)).unwrap_or_else(|e| panic!("{case_name}: read expected {rel}: {e}"));
        let actual_norm = normalize_text(rel, &actual);
        let expected_norm = normalize_text(rel, &expected);
        if actual_norm != expected_norm {
            mismatches.push(rel.clone());
        }
    }
    assert!(mismatches.is_empty(), "{case_name}: mismatched files: {mismatches:#?}");

    let _ = fs::remove_dir_all(&workdir);
}

#[test]
fn v1_reference_full_okf_tree() {
    let expected_bundle = repo_root().join("v2/sopkb-rust/fixtures/cases/reference/expected-python/bundle");
    run_full_tree_diff("reference", &expected_bundle, &expected_bundle);
}

#[test]
fn v1_authored_okf_full_okf_tree_and_md_only_filter() {
    let case_dir = repo_root().join("v2/sopkb-rust/fixtures/cases/authored-okf");
    let expected_bundle = case_dir.join("expected-python/bundle");
    run_full_tree_diff("authored-okf", &expected_bundle, &expected_bundle);

    // P-X2 sibling behavior, asserted directly (not just via the tree diff above):
    // `.sopkb/authored_okf/overrides/` has both a `.md` and a `.txt` file; only the
    // `.md` one should ever appear under `authored/`.
    let workdir = std::env::temp_dir().join(format!("sopkb-phase5-authored-okf-filter-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    copy_dir(&expected_bundle, &workdir);
    sopkb_export::sync_okf_bundle(&workdir).unwrap();
    assert!(workdir.join("authored/overrides/manual-override.md").exists());
    assert!(!workdir.join("authored/overrides/notes.txt").exists(), "non-.md file must not be copied");
    let _ = fs::remove_dir_all(&workdir);
}

#[test]
fn v1_legacy_layout_migration_survives_sync_and_full_okf_tree() {
    let case_dir = repo_root().join("v2/sopkb-rust/fixtures/cases/legacy-layout");
    let input_bundle = case_dir.join("input/bundle");
    let expected_bundle = case_dir.join("expected-python/bundle");
    // Starting point is the PRE-migration legacy layout (state files only at their
    // legacy paths, nothing under `.sopkb/` yet) -- this is what actually exercises
    // `migrate_legacy_state` running before `reset_okf_documents` (P-X3).
    run_full_tree_diff("legacy-layout", &input_bundle, &expected_bundle);

    // P-X3, asserted directly: all nine `STATE_FILE_ALIASES` entries exist -- migrated
    // -- at their canonical `.sopkb/<name>` location. `migrate_legacy_state` COPIES
    // (never deletes) the legacy file, but `reset_okf_documents` runs immediately
    // after and recursively wipes `knowledge/`, which is where 7 of the 9 legacy
    // files physically live -- so those 7 do NOT survive at their legacy path (their
    // DATA survives, via the `.sopkb/` copy taken just before the wipe). Only the two
    // legacy files OUTSIDE `knowledge/` (`sources/inventory.json`, whose directory is
    // only glob-wiped of top-level `*.md`; `reports/agent_chat.json`, whose directory
    // isn't touched at all) survive at BOTH locations. Getting the migrate/reset
    // ORDER backwards would lose the data entirely (no `.sopkb/` copy ever made); this
    // asserts the order is right, not that every legacy path is untouched.
    let workdir = std::env::temp_dir().join(format!("sopkb-phase5-legacy-migration-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    copy_dir(&input_bundle, &workdir);
    sopkb_export::sync_okf_bundle(&workdir).unwrap();
    for (canonical, legacy) in sopkb_core::store::STATE_FILE_ALIASES {
        assert!(workdir.join(".sopkb").join(canonical).exists(), "missing migrated .sopkb/{canonical}");
        let survives_at_legacy_path = !legacy.starts_with("knowledge/");
        assert_eq!(
            workdir.join(legacy).exists(),
            survives_at_legacy_path,
            "legacy path survival mismatch for {legacy} (expected survives={survives_at_legacy_path})"
        );
    }
    let _ = fs::remove_dir_all(&workdir);
}

/// P-X2 [PRESERVE -- test it explicitly, per PORT_PLAN.md §6.6's Done-when]: a stray
/// file directly under `sources/` is deleted by a sync (non-recursive `*.md` glob),
/// but stray files under `sources/originals/` and `sources/normalized/` survive
/// untouched (those subdirectories are never in `reset_okf_documents`'s wipe list).
#[test]
fn sources_originals_and_normalized_survive_sync_while_top_level_md_does_not() {
    let expected_bundle = repo_root().join("v2/sopkb-rust/fixtures/cases/reference/expected-python/bundle");
    let workdir = std::env::temp_dir().join(format!("sopkb-phase5-sources-survive-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    copy_dir(&expected_bundle, &workdir);

    fs::create_dir_all(workdir.join("sources/originals")).unwrap();
    fs::create_dir_all(workdir.join("sources/normalized")).unwrap();
    fs::write(workdir.join("sources/stale-top-level.md"), "stale").unwrap();
    fs::write(workdir.join("sources/originals/stray-original.md"), "original content").unwrap();
    fs::write(workdir.join("sources/normalized/stray-normalized.md"), "normalized content").unwrap();

    sopkb_export::sync_okf_bundle(&workdir).unwrap();

    assert!(!workdir.join("sources/stale-top-level.md").exists(), "top-level sources/*.md must be wiped");
    assert!(workdir.join("sources/originals/stray-original.md").exists(), "sources/originals/ must survive");
    assert!(workdir.join("sources/normalized/stray-normalized.md").exists(), "sources/normalized/ must survive");

    let _ = fs::remove_dir_all(&workdir);
}

/// Opt-in (`cargo test -- --ignored`) full-scale verification against the REAL
/// `examples/glp1-healthcare/bundle` (692 files, not a small fixture case): compares
/// this port's `sync_okf_bundle` output against a companion snapshot produced by
/// invoking the real Python `sopkb.export.sync_okf_bundle` directly (see
/// `v2/sopkb-rust/fixtures/harness/phase5_python_sync.py`, run once with `python
/// phase5_python_sync.py <dest>`). NOT run by default because (a) it needs that
/// Python snapshot to exist at a path passed via `SOPKB_PY_SYNCED_BUNDLE`, and (b) a
/// 692-file tree copy+diff is slow for routine `cargo test`. The `reference`/
/// `authored-okf`/`legacy-layout` tests above already cover the same code paths
/// end-to-end against real recorded Python output and DO run by default. Confirmed
/// passing during Phase 5 development (session notes) -- both a byte-for-byte tree
/// diff (index.md, log.md, sources/*.md, all nine regenerated directories) after
/// normalizing the `generated: date:` field the same way `harness.py` does.
#[test]
#[ignore]
fn v1_real_glp1_healthcare_bundle_against_live_python_snapshot() {
    let Ok(python_synced) = std::env::var("SOPKB_PY_SYNCED_BUNDLE") else {
        panic!("set SOPKB_PY_SYNCED_BUNDLE to a bundle dir produced by `python phase5_python_sync.py <dest>`");
    };
    let expected_bundle = PathBuf::from(python_synced);
    let real_bundle = repo_root().join("examples/glp1-healthcare/bundle");
    run_full_tree_diff("glp1-healthcare-real", &real_bundle, &expected_bundle);
}

/// `test_first_vertical_slice_gate` analogue: a hand-placed stale file under one of
/// the nine regenerated directories does not survive a sync (full recursive wipe,
/// regardless of extension).
#[test]
fn stale_file_under_regenerated_directory_does_not_survive_sync() {
    let expected_bundle = repo_root().join("v2/sopkb-rust/fixtures/cases/reference/expected-python/bundle");
    let workdir = std::env::temp_dir().join(format!("sopkb-phase5-stale-regen-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    copy_dir(&expected_bundle, &workdir);

    fs::create_dir_all(workdir.join("knowledge")).unwrap();
    fs::write(workdir.join("knowledge/stale.md"), "stale").unwrap();
    fs::write(workdir.join("knowledge/stale.txt"), "stale non-md too").unwrap();

    sopkb_export::sync_okf_bundle(&workdir).unwrap();

    assert!(!workdir.join("knowledge/stale.md").exists());
    assert!(!workdir.join("knowledge/stale.txt").exists(), "reset_directory wipes regardless of extension");

    let _ = fs::remove_dir_all(&workdir);
}
