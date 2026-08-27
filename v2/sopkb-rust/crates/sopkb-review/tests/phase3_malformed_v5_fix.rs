//! Phase 3 acceptance gate, P-V5 half (docs/port/PORT_PLAN.md §6.4 Done-when): "The
//! malformed-* cases must produce validation errors rather than crashing." Confirmed
//! against the real fixture corpus that Python currently DOES crash on all five
//! (uncaught TypeError/AttributeError, exit 1, no reports/ written at all -- see each
//! case's `expected-python/_transcript.txt`). This is a deliberate FIX
//! (docs/port/PORT_PLAN.md §6.0's fix-tracking requirement -- see DEVIATIONS.md for
//! the corresponding entries), not a byte-for-byte V1 diff against Python's (crashed)
//! output: there is nothing to diff against. The gate here is "does not panic, and
//! produces a non-empty, sensible error list instead."

use sopkb_review::validate::validate_bundle;
use std::fs;
use std::path::PathBuf;

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

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
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

fn run_case(case_name: &str, expected_error_substring: &str) {
    let input_bundle = repo_root().join("v2/sopkb-rust/fixtures/cases").join(case_name).join("input/bundle");
    assert!(input_bundle.exists(), "missing input/bundle for {case_name}");

    let workdir = std::env::temp_dir().join(format!("sopkb-phase3-{case_name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    copy_dir(&input_bundle, &workdir);

    // The gate itself: this must not panic (Python's equivalent does crash today).
    let (errors, _warnings) = validate_bundle(&workdir).unwrap_or_else(|e| panic!("{case_name}: unexpected I/O error: {e}"));

    assert!(
        !errors.is_empty(),
        "{case_name}: expected a non-empty error list flagging the malformed state, got none"
    );
    assert!(
        errors.iter().any(|e| e.contains(expected_error_substring)),
        "{case_name}: expected an error containing {expected_error_substring:?}, got {errors:?}"
    );
    assert!(
        workdir.join("reports/validation.json").exists(),
        "{case_name}: unlike Python (which crashes before writing anything), the Rust port must still write reports/validation.json"
    );

    let _ = fs::remove_dir_all(&workdir);
}

#[test]
fn malformed_null_manifest_sources_does_not_crash() {
    // manifest.yaml `sources: null` vs a real (non-empty) inventory.json -> the id-set
    // comparison naturally reports a mismatch instead of Python's `TypeError: 'NoneType'
    // object is not iterable`.
    run_case("malformed-null-manifest-sources", "manifest sources do not match inventory sources");
}

#[test]
fn malformed_null_confidence_does_not_crash() {
    // Python: `None < 0.7` raises TypeError. Rust: `.as_f64()` on a JSON null returns
    // None, defaulting to 1.0 (the Python default-when-absent value) -- so this
    // specific malformed case produces zero errors/warnings about confidence, which is
    // arguably too lenient (a present-but-null confidence silently passes as if it
    // were absent) but it is a considered, non-crashing choice, not an oversight.
    let case_name = "malformed-null-confidence";
    let input_bundle = repo_root().join("v2/sopkb-rust/fixtures/cases").join(case_name).join("input/bundle");
    let workdir = std::env::temp_dir().join(format!("sopkb-phase3-{case_name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    copy_dir(&input_bundle, &workdir);
    let result = validate_bundle(&workdir);
    assert!(result.is_ok(), "{case_name}: must not crash, got {result:?}");
    assert!(workdir.join("reports/validation.json").exists());
    let _ = fs::remove_dir_all(&workdir);
}

#[test]
fn malformed_items_object_does_not_crash() {
    // items.json is a JSON object, not an array -- `.as_array()` returns None, so the
    // per-item loop (`items.as_array().into_iter().flatten()`) simply iterates zero
    // times rather than Python's `AttributeError: 'str' object has no attribute 'get'`
    // (which fires while iterating dict keys as if they were item dicts).
    let case_name = "malformed-items-object";
    let input_bundle = repo_root().join("v2/sopkb-rust/fixtures/cases").join(case_name).join("input/bundle");
    let workdir = std::env::temp_dir().join(format!("sopkb-phase3-{case_name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    copy_dir(&input_bundle, &workdir);
    let result = validate_bundle(&workdir);
    assert!(result.is_ok(), "{case_name}: must not crash, got {result:?}");
    assert!(workdir.join("reports/validation.json").exists());
    let _ = fs::remove_dir_all(&workdir);
}

#[test]
fn malformed_inventory_array_does_not_crash() {
    run_case("malformed-inventory-array", "manifest sources do not match inventory sources");
}

#[test]
fn malformed_warnings_strings_does_not_crash() {
    // inventory.json's `warnings` array holds bare strings, not `{path, warning}`
    // objects. `w["path"]`/`w["warning"]` on a JSON string via serde_json's `Index`
    // impl yields `Value::Null` (not a panic -- unlike Python's `AttributeError: 'str'
    // object has no attribute 'get'`), which `string_or_none` renders as the literal
    // "None", matching the f-string-interpolates-a-missing-key precedent the port
    // mapping doc calls out for the analogous review-id case.
    let case_name = "malformed-warnings-strings";
    let input_bundle = repo_root().join("v2/sopkb-rust/fixtures/cases").join(case_name).join("input/bundle");
    let workdir = std::env::temp_dir().join(format!("sopkb-phase3-{case_name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    copy_dir(&input_bundle, &workdir);
    let (_errors, warnings) = validate_bundle(&workdir).unwrap_or_else(|e| panic!("{case_name}: unexpected I/O error: {e}"));
    assert!(warnings.iter().any(|w| w.contains("inventory warning: None - None")));
    assert!(workdir.join("reports/validation.json").exists());
    let _ = fs::remove_dir_all(&workdir);
}
