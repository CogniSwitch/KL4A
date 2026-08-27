//! Integration tests for the user-facing `sopkb-cli` surface: argument parsing (clear
//! usage errors, not panics, for malformed invocations), the exit-code contract
//! (`validate` returns 1 iff `errors` is non-empty), JSON output shape spot-checks, and
//! the reviewer-default-resolution behavior (DEVIATIONS.md P-R6).
//!
//! `v2/sopkb-rust/fixtures/harness/harness.py` (invoked via Python, not available in every
//! dev environment) covers byte-exact-vs-Python behavior for the pipeline commands
//! separately; these tests exercise this crate's own dispatch/argument-parsing layer
//! directly, without spawning the compiled binary.

use sopkb_cli::cli::{BundleCommand, Command, ConflictsCommand, ReviewActionArgs, ReviewCommand, SourcesCommand};
use sopkb_cli::{execute, run, RunError};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn new_bundle() -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let bundle_dir = dir.path().join("bundle");
    sopkb_core::store::create_bundle(&bundle_dir, Some("Test Bundle")).unwrap();
    (dir, bundle_dir)
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn with_settings_path<F: FnOnce()>(f: F) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");
    unsafe { std::env::set_var("SOPKB_SETTINGS_PATH", &path) };
    f();
    unsafe { std::env::remove_var("SOPKB_SETTINGS_PATH") };
}

fn repo_root() -> PathBuf {
    // Walk up from this crate's own directory until a top-level `.git` entry is
    // found (a directory in a normal checkout, a file in a git worktree) --
    // depth-agnostic, unlike a fixed `ancestors().nth(N)` hop count.
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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

// --- argument parsing: clear usage errors, not panics --------------------------------

#[test]
fn missing_required_rationale_is_a_usage_error_not_a_panic() {
    let err = run(["sopkb-cli", "review", "approve", "dir", "item"]).unwrap_err();
    assert!(matches!(err, RunError::Usage(_)), "expected a clap usage error, got {err:?}");
}

#[test]
fn missing_bundle_dir_positional_is_a_usage_error() {
    let err = run(["sopkb-cli", "validate"]).unwrap_err();
    assert!(matches!(err, RunError::Usage(_)));
}

#[test]
fn unknown_subcommand_is_a_usage_error() {
    let err = run(["sopkb-cli", "not-a-real-command", "x"]).unwrap_err();
    assert!(matches!(err, RunError::Usage(_)));
}

#[test]
fn missing_agent_context_task_flag_is_a_usage_error() {
    let err = run(["sopkb-cli", "agent", "context", "dir"]).unwrap_err();
    assert!(matches!(err, RunError::Usage(_)));
}

#[test]
fn valid_invocation_parses_and_runs() {
    let (_dir, bundle_dir) = new_bundle();
    let success = run(["sopkb-cli", "bundle", "describe", &s(&bundle_dir)]).unwrap();
    assert_eq!(success.exit_code, 0);
    assert!(success.text.contains("\"id\""));
}

// --- validate exit-code contract -------------------------------------------------------

#[test]
fn validate_exits_zero_when_no_errors() {
    let (_dir, bundle_dir) = new_bundle();
    let success = execute(Command::Validate { bundle_dir: s(&bundle_dir) }).unwrap();
    assert_eq!(success.exit_code, 0);
    assert!(success.text.starts_with("Validation completed with 0 error(s)"), "{}", success.text);
}

#[test]
fn validate_exits_one_when_errors_present() {
    // Reuse the malformed-null-manifest-sources fixture (also exercised by
    // sopkb-review's own P-V5 gate tests): manifest.yaml's `sources: null` disagrees
    // with a non-empty inventory.json, which validate_bundle reports as an error
    // ("manifest sources do not match inventory sources") rather than crashing.
    let input_bundle = repo_root().join("v2/sopkb-rust/fixtures/cases/malformed-null-manifest-sources/input/bundle");
    assert!(input_bundle.exists(), "fixture moved or missing: {}", input_bundle.display());
    let workdir = std::env::temp_dir().join(format!("sopkb-cli-validate-exit1-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    copy_dir(&input_bundle, &workdir);

    let success = execute(Command::Validate { bundle_dir: s(&workdir) }).unwrap();
    assert_eq!(success.exit_code, 1, "{}", success.text);
    assert!(!success.text.starts_with("Validation completed with 0 error(s)"), "{}", success.text);

    let _ = fs::remove_dir_all(&workdir);
}

#[test]
fn validate_on_missing_bundle_reports_it_as_an_error_not_a_crash() {
    // validate_bundle catches a manifest load failure itself and reports it as the
    // (only) validation error rather than propagating -- so this is still the
    // exit-code-1 path, distinct from a true backend-error propagation (see
    // `bundle_describe_on_missing_bundle_is_a_backend_error_not_a_panic` below).
    let missing = std::env::temp_dir().join("sopkb-cli-definitely-does-not-exist-xyz");
    let success = execute(Command::Validate { bundle_dir: s(&missing) }).unwrap();
    assert_eq!(success.exit_code, 1);
}

#[test]
fn bundle_describe_on_missing_bundle_is_a_backend_error_not_a_panic() {
    let missing = std::env::temp_dir().join("sopkb-cli-definitely-does-not-exist-xyz");
    let err = execute(Command::Bundle { command: BundleCommand::Describe { bundle_dir: s(&missing) } }).unwrap_err();
    assert!(err.to_string().contains("Missing manifest"), "{err}");
}

// --- JSON output shape spot-checks ------------------------------------------------------

#[test]
fn bundle_describe_prints_canonical_sorted_json() {
    let (_dir, bundle_dir) = new_bundle();
    let success = execute(Command::Bundle { command: BundleCommand::Describe { bundle_dir: s(&bundle_dir) } }).unwrap();
    // sopkb_fmt::to_canonical_json: indent=2, sorted keys -- "id" sorts before "title".
    let id_pos = success.text.find("\"id\"").unwrap();
    let title_pos = success.text.find("\"title\"").unwrap();
    assert!(id_pos < title_pos, "expected sort_keys=True ordering, got:\n{}", success.text);
    let parsed: serde_json::Value = serde_json::from_str(&success.text).unwrap();
    assert_eq!(parsed["title"], serde_json::json!("Test Bundle"));
}

#[test]
fn sources_list_on_fresh_bundle_is_an_empty_json_array() {
    let (_dir, bundle_dir) = new_bundle();
    let success = execute(Command::Sources { command: SourcesCommand::List { bundle_dir: s(&bundle_dir) } }).unwrap();
    assert_eq!(success.text, "[]");
}

#[test]
fn conflicts_list_prints_raw_markdown_not_json() {
    let (_dir, bundle_dir) = new_bundle();
    let success = execute(Command::Conflicts { command: ConflictsCommand::List { bundle_dir: s(&bundle_dir) } }).unwrap();
    assert!(success.text.starts_with("# Conflict Report"), "{}", success.text);
    assert!(!success.text.trim_start().starts_with('{'));
}

// --- reviewer default resolution (DEVIATIONS.md P-R6) ----------------------------------

#[test]
#[serial_test::serial]
fn reviewer_omitted_and_unconfigured_falls_back_to_local_user_p_r6() {
    with_settings_path(|| {
        let (_dir, bundle_dir) = new_bundle();
        let items = serde_json::json!([{
            "id": "ki-1", "subject": "A", "predicate": "requires", "object": "B",
            "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.9,
            "source_text": "T", "span_status": "exact", "start_pos": 0, "end_pos": 1
        }]);
        sopkb_core::store::write_state_json(&bundle_dir, "items.json", &items).unwrap();

        let args = ReviewActionArgs {
            bundle_dir: s(&bundle_dir),
            knowledge_item_id: "ki-1".to_string(),
            rationale: "looks fine".to_string(),
            reviewer: None,
        };
        let success = execute(Command::Review { command: ReviewCommand::Approve(args) }).unwrap();
        let event: serde_json::Value = serde_json::from_str(&success.text).unwrap();
        assert_eq!(event["reviewer"], serde_json::json!("local:user"));
    });
}

#[test]
#[serial_test::serial]
fn reviewer_omitted_falls_back_to_configured_settings_name_p_r6() {
    with_settings_path(|| {
        sopkb_config::set_reviewer_name("dr:jane").unwrap();

        let (_dir, bundle_dir) = new_bundle();
        let items = serde_json::json!([{
            "id": "ki-1", "subject": "A", "predicate": "requires", "object": "B",
            "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.9,
            "source_text": "T", "span_status": "exact", "start_pos": 0, "end_pos": 1
        }]);
        sopkb_core::store::write_state_json(&bundle_dir, "items.json", &items).unwrap();

        let args = ReviewActionArgs {
            bundle_dir: s(&bundle_dir),
            knowledge_item_id: "ki-1".to_string(),
            rationale: "looks fine".to_string(),
            reviewer: None,
        };
        let success = execute(Command::Review { command: ReviewCommand::Approve(args) }).unwrap();
        let event: serde_json::Value = serde_json::from_str(&success.text).unwrap();
        // Unlike the ORIGINAL Python CLI (which would have hardcoded "local:user" here,
        // ignoring the configured reviewer name entirely), this port's CLI now agrees
        // with the web path and picks up the configured settings reviewer name.
        assert_eq!(event["reviewer"], serde_json::json!("dr:jane"));
    });
}

#[test]
#[serial_test::serial]
fn explicit_reviewer_flag_is_never_overridden_p_r6() {
    with_settings_path(|| {
        sopkb_config::set_reviewer_name("dr:jane").unwrap();

        let (_dir, bundle_dir) = new_bundle();
        let items = serde_json::json!([{
            "id": "ki-1", "subject": "A", "predicate": "requires", "object": "B",
            "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.9,
            "source_text": "T", "span_status": "exact", "start_pos": 0, "end_pos": 1
        }]);
        sopkb_core::store::write_state_json(&bundle_dir, "items.json", &items).unwrap();

        let args = ReviewActionArgs {
            bundle_dir: s(&bundle_dir),
            knowledge_item_id: "ki-1".to_string(),
            rationale: "looks fine".to_string(),
            reviewer: Some("local:reviewer-a".to_string()),
        };
        let success = execute(Command::Review { command: ReviewCommand::Approve(args) }).unwrap();
        let event: serde_json::Value = serde_json::from_str(&success.text).unwrap();
        assert_eq!(event["reviewer"], serde_json::json!("local:reviewer-a"));
    });
}
