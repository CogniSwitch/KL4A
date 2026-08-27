//! Ingest pipeline orchestration. docs/port/port-mapping-e-web-cli-contract.md §2.1
//! (`handle_ingest_form`, `web_app.py:446-475`); PORT_PLAN.md §4.3, P-W21;
//! docs/port/DECISIONS.md Q6a, Q8.
//!
//! **Strictly sequential, short-circuiting on the first error** -- this matches
//! Python's own control flow exactly: `handle_ingest_form` is a single function body
//! where each step's call sits directly on a line (`result["items"] = len(mine_bundle(...))`);
//! an exception from any step propagates immediately out of the whole function, so no
//! step after the failing one ever runs, and step 5's `sync_okf_bundle` never executes
//! either unless every requested step up to that point already succeeded. This is not
//! a design choice made independently here -- it falls directly out of porting the
//! Python control flow with `?` on each step.

use serde_json::Value;
use sopkb_core::error::{Result, SopkbError};
use std::path::{Path, PathBuf};

/// P-W16 FIX (no bytes cross the boundary) + docs/port/DECISIONS.md Q8 (folder picks
/// bypass staging entirely and point `scan_sources` straight at the picked directory).
#[derive(Debug, Clone)]
pub enum IngestSource {
    /// Use whatever is currently staged at `bundle/.sopkb/uploads/current` (see
    /// `crate::upload::stage_uploaded_files`).
    Staged,
    /// An explicit folder path, bypassing upload staging entirely (Q8).
    Folder(PathBuf),
}

/// docs/port/port-mapping-e-web-cli-contract.md §4.3:
/// ```text
/// IngestRequest {
///     source: { kind: "staged" } | { kind: "folder", path: string }
///     scan, normalize, mine, validate, export : bool
///     mine_provider: "fixture" | "azure-llm"
///     profile_id: string?
/// }
/// ```
/// There is deliberately no `Default` implementation carrying docs/port/DECISIONS.md
/// Q6a's `"azure-llm"` default -- `sopkb_mining::mine_bundle` itself has no implicit
/// default parameter either, and the actual product-level default belongs to whatever
/// caller renders the ingest form, not to this orchestration layer.
#[derive(Debug, Clone)]
pub struct IngestRequest {
    pub source: IngestSource,
    pub scan: bool,
    pub normalize: bool,
    pub mine: bool,
    pub validate: bool,
    pub export: bool,
    pub mine_provider: String,
    pub profile_id: Option<String>,
    /// Echoed back as `IngestResult.uploaded_files` when set -- mirrors Python's
    /// `form["_uploaded_file_count"]`, which `stage_uploaded_files`'s caller injects
    /// from a PRIOR staging call. This orchestration layer doesn't track staging state
    /// itself, so the caller (which just called `stage_uploaded_files`) supplies it.
    pub uploaded_file_count: Option<usize>,
}

/// The one place `IngestSource` becomes a concrete directory, shared by
/// [`run_ingest_pipeline`] and [`preview_ingest_sources`] so a preview can never resolve
/// to a different directory than the run it precedes.
fn resolve_source_dir(bundle_dir: &Path, request: &IngestRequest) -> PathBuf {
    match &request.source {
        IngestSource::Folder(path) => path.clone(),
        IngestSource::Staged => sopkb_core::store::state_path(bundle_dir, "uploads").join("current"),
    }
}

/// P-W21 PRESERVE: `handle_ingest_form` hardcodes these two formats regardless of what
/// a hypothetical "export screen" checkbox state might be -- made visible here as a
/// named constant rather than buried inline, per PORT_PLAN.md's explicit instruction.
pub const DEFAULT_EXPORT_FORMATS: &[&str] = &["graph-json", "rdf"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationCounts {
    pub errors: usize,
    pub warnings: usize,
}

#[derive(Debug, Clone, Default)]
pub struct IngestResult {
    pub uploaded_files: Option<usize>,
    pub sources: Option<usize>,
    pub sections: Option<usize>,
    pub items: Option<usize>,
    pub mine_provider: Option<String>,
    pub validation: Option<ValidationCounts>,
    pub okf_bundle: Option<Value>,
    pub exports: Option<Vec<Value>>,
}

/// `handle_ingest_form` (`web_app.py:446-475`). Wraps the whole step sequence in
/// `sopkb_review::with_bundle_lock` (PORT_PLAN.md's own instruction: "Wrap every
/// bundle-mutating operation you orchestrate... in this") -- every step here mutates
/// bundle state on disk, and none of `scan_sources`/`normalize_sources`/`mine_bundle`/
/// `validate_bundle`/`sync_okf_bundle`/`export_bundle` take the lock themselves (only
/// `sopkb-review`'s five review actions do), so this orchestration layer is the one
/// place that must.
pub fn run_ingest_pipeline(bundle_dir: &Path, request: &IngestRequest) -> Result<IngestResult> {
    let source_dir = resolve_source_dir(bundle_dir, request);

    // Validation, in order (`web_app.py:447-450`). The "missing required field:
    // source_dir" half of Python's check has no Rust equivalent to reproduce: it
    // exists only because Python's `source_dir`/`_uploaded_source_dir` are both
    // untyped, possibly-absent form strings, whereas `IngestSource` structurally
    // guarantees a path is always present for both variants.
    if !source_dir.exists() || !source_dir.is_dir() {
        return Err(SopkbError::NotFound(format!("source directory does not exist: {}", source_dir.display())));
    }

    sopkb_review::with_bundle_lock(bundle_dir, || run_ingest_pipeline_locked(bundle_dir, &source_dir, request))
}

fn run_ingest_pipeline_locked(bundle_dir: &Path, source_dir: &Path, request: &IngestRequest) -> Result<IngestResult> {
    let mut result = IngestResult { uploaded_files: request.uploaded_file_count, ..Default::default() };
    let mut any_ran = false;

    if request.scan {
        let sources = sopkb_core::inventory::scan_sources(source_dir, bundle_dir)?;
        result.sources = Some(sources.len());
        any_ran = true;
    }
    if request.normalize {
        // Reuses the same provider/profile the run already carries for mining
        // (`request.mine_provider`/`profile_id`) rather than adding a second,
        // independent provider choice just for normalize -- oss-launch's own
        // ingest form has a single provider selector governing the whole run,
        // not one per step. See `heading_restructure::provider_hook`'s doc
        // comment for why this closure is how sopkb-core stays free of an
        // sopkb-llm dependency while still supporting LLM-based heading
        // restructuring here.
        let log_warning = |message: &str| sopkb_core::store::append_ingest_log(bundle_dir, message);
        let restructure =
            crate::heading_restructure::provider_hook(&request.mine_provider, request.profile_id.as_deref(), None, None, Some(&log_warning));
        let sections = sopkb_core::normalize::normalize_sources(bundle_dir, restructure.as_deref(), Some(sopkb_config::max_parallel_workers()))?;
        result.sections = Some(sections.len());
        any_ran = true;
    }
    if request.mine {
        let items = sopkb_mining::mine_bundle(bundle_dir, &request.mine_provider, request.profile_id.as_deref(), None, None)?;
        result.items = Some(items.len());
        result.mine_provider = Some(request.mine_provider.clone());
        any_ran = true;
    }
    if request.validate {
        let (errors, warnings) = sopkb_review::validate_bundle(bundle_dir)?;
        result.validation = Some(ValidationCounts { errors: errors.len(), warnings: warnings.len() });
        any_ran = true;
    }
    // Step 5, derived (NOT its own flag): sync runs if ANY of the four steps above
    // ran, gated purely on "did a state-changing step run", matching Python's
    // `if any(form.get(step)=="on" for step in [...]): result["okf_bundle"] = sync_okf_bundle(...)`.
    if any_ran {
        let okf_bundle = sopkb_export::sync_okf_bundle(bundle_dir)?;
        result.okf_bundle = Some(okf_bundle);
    }
    if request.export {
        let formats: Vec<String> = DEFAULT_EXPORT_FORMATS.iter().map(|s| s.to_string()).collect();
        let exports = sopkb_export::export_bundle(bundle_dir, &formats)?;
        result.exports = Some(exports);
    }

    Ok(result)
}

/// A read-only dry run of the scan step: what each file in the ingest source would do
/// to the bundle (new source / new version / unchanged / unsupported), with nothing
/// written. Backs a confirm-before-ingest preview -- `handle_ingest_form`'s Python
/// counterpart reaches `classify_source_updates` the same way.
///
/// Resolves `request.source` exactly as [`run_ingest_pipeline`] does, so the preview and
/// the run it precedes can never disagree about which directory is being ingested.
/// Takes no bundle lock: it is a pure read, and holding the lock across a
/// user-facing confirmation step would block every other operation on the bundle for as
/// long as the user takes to decide.
pub fn preview_ingest_sources(bundle_dir: &Path, request: &IngestRequest) -> Result<Value> {
    let source_dir = resolve_source_dir(bundle_dir, request);
    if !source_dir.exists() || !source_dir.is_dir() {
        return Err(SopkbError::NotFound(format!("source directory does not exist: {}", source_dir.display())));
    }
    sopkb_core::inventory::classify_source_updates(&source_dir, bundle_dir)
}

/// Retires a source and resyncs the OKF document tree, under the bundle lock.
///
/// The retire itself is already all-or-nothing (see
/// `sopkb_review::source_lifecycle::retire_source`); the lock here is what stops a
/// concurrent ingest from interleaving with it, and the resync afterwards is what makes
/// the retirement visible in the generated documents. Both mirror what every other
/// mutating path in this orchestration layer does.
pub fn retire_bundle_source(
    bundle_dir: &Path,
    source_id: &str,
    actor: &str,
    rationale: &str,
) -> Result<sopkb_review::RetireOutcome> {
    let outcome = sopkb_review::with_bundle_lock(bundle_dir, || {
        sopkb_review::retire_source(bundle_dir, source_id, actor, rationale)
    })?;
    // Only resync when something actually changed -- a repeated retire stays inert all
    // the way out to the document tree, rather than rewriting every OKF file to
    // byte-identical content and bumping the manifest's `updated_at`.
    if outcome.did_change() {
        sopkb_export::sync_okf_bundle(bundle_dir)?;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;

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

    fn reference_source_dir() -> PathBuf {
        repo_root().join("v2/sopkb-rust/fixtures/cases/reference/input/sources")
    }

    fn fresh_bundle() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        sopkb_core::store::create_bundle(&bundle_dir, Some("Pipeline Bundle")).unwrap();
        (dir, bundle_dir)
    }

    fn base_request(source_dir: PathBuf) -> IngestRequest {
        IngestRequest {
            source: IngestSource::Folder(source_dir),
            scan: false,
            normalize: false,
            mine: false,
            validate: false,
            export: false,
            mine_provider: "fixture".to_string(),
            profile_id: None,
            uploaded_file_count: None,
        }
    }

    #[test]
    fn missing_source_dir_is_not_found() {
        let (_dir, bundle_dir) = fresh_bundle();
        let missing = Path::new("this/does/not/exist/anywhere").to_path_buf();
        let err = run_ingest_pipeline(&bundle_dir, &base_request(missing)).unwrap_err();
        assert!(err.to_string().starts_with("source directory does not exist:"));
    }

    #[test]
    fn full_pipeline_runs_steps_in_order_and_produces_expected_counts() {
        let (_dir, bundle_dir) = fresh_bundle();
        let mut request = base_request(reference_source_dir());
        request.scan = true;
        request.normalize = true;
        request.mine = true;
        request.validate = true;
        request.export = true;

        let result = run_ingest_pipeline(&bundle_dir, &request).unwrap();
        assert_eq!(result.sources, Some(4));
        assert!(result.sections.unwrap() > 0);
        assert!(result.items.unwrap() > 0);
        assert_eq!(result.mine_provider.as_deref(), Some("fixture"));
        let validation = result.validation.unwrap();
        assert_eq!(validation.errors, 0);
        assert_eq!(result.okf_bundle, Some(serde_json::json!({"type": "okf_native", "path": "."})));
        assert_eq!(result.exports.as_ref().unwrap().len(), 2);
        assert!(bundle_dir.join("index.md").exists());
    }

    #[test]
    fn sync_runs_without_export_flag_but_export_artifacts_absent() {
        let (_dir, bundle_dir) = fresh_bundle();
        let mut request = base_request(reference_source_dir());
        request.scan = true;
        request.normalize = true;
        request.mine = true;
        request.validate = true;
        // export left false

        let result = run_ingest_pipeline(&bundle_dir, &request).unwrap();
        assert_eq!(result.okf_bundle, Some(serde_json::json!({"type": "okf_native", "path": "."})));
        assert!(result.exports.is_none());
        assert!(bundle_dir.join("index.md").exists());
        assert!(!sopkb_export::default_export_dir(&bundle_dir).unwrap().join("graph").join("graph.json").exists());
    }

    #[test]
    fn no_steps_selected_never_syncs() {
        let (_dir, bundle_dir) = fresh_bundle();
        let request = base_request(reference_source_dir());
        let result = run_ingest_pipeline(&bundle_dir, &request).unwrap();
        assert!(result.okf_bundle.is_none());
        assert!(result.sources.is_none());
    }

    /// The explicitly-called-out gap case: normalize with no prior scan in THIS run.
    /// Python reproduces this rather than guarding against it -- `normalize_sources`
    /// just operates on whatever `inventory.json` currently says (absent here, since
    /// this is a fresh bundle), which is a legal, if unusual, no-op-ish call.
    #[test]
    fn normalize_without_scan_reproduces_pythons_gap_case() {
        let (_dir, bundle_dir) = fresh_bundle();
        let mut request = base_request(reference_source_dir());
        request.normalize = true;
        let result = run_ingest_pipeline(&bundle_dir, &request).unwrap();
        // No inventory sources recorded yet -> zero sections, but NOT an error.
        assert_eq!(result.sections, Some(0));
        assert!(result.okf_bundle.is_some(), "step 5 still runs because normalize (flag) ran");
    }

    /// Exhaustive step-order/gating contract across all 2^5 flag combinations, using
    /// the deterministic "fixture" provider (no network) -- proves exactly the
    /// ordering and step-5-derivation contract from
    /// docs/port/port-mapping-e-web-cli-contract.md §2.1's table, which PORT_PLAN.md's
    /// Phase 9 "Done when" criteria names explicitly.
    #[test]
    fn all_32_flag_combinations_respect_the_documented_step_order_and_gating() {
        for mask in 0u8..32 {
            let scan = mask & 1 != 0;
            let normalize = mask & 2 != 0;
            let mine = mask & 4 != 0;
            let validate = mask & 8 != 0;
            let export = mask & 16 != 0;

            let (_dir, bundle_dir) = fresh_bundle();
            let mut request = base_request(reference_source_dir());
            request.scan = scan;
            request.normalize = normalize;
            request.mine = mine;
            request.validate = validate;
            request.export = export;

            let result = run_ingest_pipeline(&bundle_dir, &request)
                .unwrap_or_else(|e| panic!("mask {mask:05b} (scan={scan} normalize={normalize} mine={mine} validate={validate} export={export}) failed: {e}"));

            assert_eq!(result.sources.is_some(), scan, "mask {mask:05b}: sources presence");
            assert_eq!(result.sections.is_some(), normalize, "mask {mask:05b}: sections presence");
            assert_eq!(result.items.is_some(), mine, "mask {mask:05b}: items presence");
            assert_eq!(result.mine_provider.is_some(), mine, "mask {mask:05b}: mine_provider presence");
            assert_eq!(result.validation.is_some(), validate, "mask {mask:05b}: validation presence");

            let any_ran = scan || normalize || mine || validate;
            assert_eq!(result.okf_bundle.is_some(), any_ran, "mask {mask:05b}: okf_bundle (derived step 5) must run iff any of scan/normalize/mine/validate ran, regardless of export");
            assert_eq!(result.exports.is_some(), export, "mask {mask:05b}: exports presence follows its own flag only");
            if export {
                assert_eq!(result.exports.as_ref().unwrap().len(), 2, "mask {mask:05b}: hardcoded graph-json + rdf (P-W21)");
            }
        }
    }

    /// End-to-end proof that the new id scheme is actually exercised by the real ingest
    /// entry point, not just by `scan_sources`' own unit tests: run the full pipeline and
    /// read the lifecycle state back off disk.
    #[test]
    fn the_full_pipeline_produces_the_new_id_scheme_and_both_lifecycle_state_files() {
        let (_dir, bundle_dir) = fresh_bundle();
        let mut request = base_request(reference_source_dir());
        request.scan = true;
        request.normalize = true;
        request.mine = true;
        request.validate = true;
        run_ingest_pipeline(&bundle_dir, &request).unwrap();

        let inventory =
            sopkb_core::store::read_state_json(&bundle_dir, "inventory.json", Value::Null).unwrap();
        for source in inventory["sources"].as_array().unwrap() {
            let id = source["id"].as_str().unwrap();
            assert!(!id.is_empty());
            // Bare stem: no 12-hex-char content-hash suffix anywhere in the id.
            assert!(
                !regex_like_hash_suffix(id),
                "source id {id} still carries a content-hash suffix -- old scheme"
            );
            assert_eq!(source["source_version_id"], format!("{id}:v1"));
            assert_eq!(source["status"], "active");
            assert_eq!(source["version_number"], 1);
            assert_eq!(source["original_path"], format!("sources/originals/{id}__v1.md"));
            assert_eq!(source["versions"].as_array().unwrap().len(), 1);
        }

        // Both new state files exist and agree with the inventory.
        let registry =
            sopkb_core::store::read_state_json(&bundle_dir, "source_versions.json", Value::Null).unwrap();
        assert_eq!(
            registry["versions"].as_array().unwrap().len(),
            inventory["sources"].as_array().unwrap().len()
        );
        let events = sopkb_core::store::read_state_json(&bundle_dir, "source_events.json", Value::Null).unwrap();
        let actions: Vec<&str> =
            events.as_array().unwrap().iter().map(|e| e["action"].as_str().unwrap()).collect();
        assert!(actions.iter().all(|a| *a == "source_added"), "a first ingest logs only source_added: {actions:?}");

        // Sections and items carry the version through, and item ids are version-keyed.
        let sections = sopkb_core::store::read_state_json(&bundle_dir, "sections.json", Value::Null).unwrap();
        for section in sections.as_array().unwrap() {
            assert_eq!(section["source_version_id"], format!("{}:v1", section["source_id"].as_str().unwrap()));
        }
        let items = sopkb_core::store::read_state_json(&bundle_dir, "items.json", Value::Null).unwrap();
        assert!(!items.as_array().unwrap().is_empty());
        for item in items.as_array().unwrap() {
            assert_eq!(item["lifecycle_status"], "active");
            let expected_prefix = format!("ki-{}-v1-", item["source_id"].as_str().unwrap());
            assert!(
                item["id"].as_str().unwrap().starts_with(&expected_prefix),
                "item id {} must be version-keyed as {expected_prefix}NNNNNN",
                item["id"]
            );
        }

        // And the report the subsystem adds is generated.
        let impact = std::fs::read_to_string(bundle_dir.join("reports/source_update_impact.md")).unwrap();
        assert!(impact.contains("Active source versions: 4"), "{impact}");
        assert!(impact.contains("Retired source versions: 0"));
    }

    /// True if `id` ends in `-<12 lowercase hex chars>`, i.e. the OLD content-hash-
    /// suffixed scheme. Spelled out rather than pulled in as a regex dependency.
    fn regex_like_hash_suffix(id: &str) -> bool {
        match id.rsplit_once('-') {
            Some((_, tail)) => tail.len() == 12 && tail.chars().all(|c| c.is_ascii_hexdigit()),
            None => false,
        }
    }

    /// Re-ingesting an edited file through the real pipeline must produce a v2, supersede
    /// v1's knowledge rather than dropping it, and keep v1's original bytes on disk.
    #[test]
    fn re_ingesting_an_edited_source_versions_it_and_supersedes_its_knowledge() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        let source_dir = dir.path().join("sources");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(source_dir.join("policy.md"), "# Eligibility\n\nStaff must confirm identity.\n").unwrap();
        sopkb_core::store::create_bundle(&bundle_dir, Some("Versioned")).unwrap();

        let mut request = base_request(source_dir.clone());
        request.scan = true;
        request.normalize = true;
        request.mine = true;
        run_ingest_pipeline(&bundle_dir, &request).unwrap();

        let first_items = sopkb_core::store::read_state_json(&bundle_dir, "items.json", Value::Null).unwrap();
        let first_ids: Vec<String> =
            first_items.as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap().to_string()).collect();
        assert!(!first_ids.is_empty());

        std::fs::write(source_dir.join("policy.md"), "# Eligibility\n\nStaff must escalate immediately.\n").unwrap();
        run_ingest_pipeline(&bundle_dir, &request).unwrap();

        let inventory = sopkb_core::store::read_state_json(&bundle_dir, "inventory.json", Value::Null).unwrap();
        let source = &inventory["sources"][0];
        assert_eq!(inventory["sources"].as_array().unwrap().len(), 1, "an edit is a version, not a new source");
        assert_eq!(source["version_number"], 2);
        assert_eq!(source["source_version_id"], "policy:v2");
        let versions = source["versions"].as_array().unwrap();
        assert_eq!(versions[0]["status"], "superseded");
        assert_eq!(versions[1]["status"], "active");
        assert!(bundle_dir.join("sources/originals/policy__v1.md").exists(), "v1's bytes must survive");
        assert!(bundle_dir.join("sources/originals/policy__v2.md").exists());

        let items = sopkb_core::store::read_state_json(&bundle_dir, "items.json", Value::Null).unwrap();
        let statuses: std::collections::BTreeMap<String, String> = items
            .as_array()
            .unwrap()
            .iter()
            .map(|i| (i["id"].as_str().unwrap().to_string(), i["lifecycle_status"].as_str().unwrap().to_string()))
            .collect();
        for id in &first_ids {
            assert_eq!(
                statuses.get(id).map(String::as_str),
                Some("superseded"),
                "v1 item {id} must be kept and marked superseded, not deleted"
            );
        }
        assert!(statuses.values().any(|s| s == "active"), "v2's items must be active");

        let events = sopkb_core::store::read_state_json(&bundle_dir, "source_events.json", Value::Null).unwrap();
        let actions: Vec<&str> = events.as_array().unwrap().iter().map(|e| e["action"].as_str().unwrap()).collect();
        assert_eq!(actions, vec!["source_added", "source_version_added"]);
    }

    #[test]
    fn preview_reports_classifications_and_writes_nothing() {
        let (_dir, bundle_dir) = fresh_bundle();
        let mut request = base_request(reference_source_dir());
        request.scan = true;

        let preview = preview_ingest_sources(&bundle_dir, &request).unwrap();
        let files = preview["files"].as_array().unwrap();
        assert_eq!(files.len(), 4);
        assert!(files.iter().all(|f| f["classification"] == "new_source"));
        assert!(
            !sopkb_core::store::state_path(&bundle_dir, "inventory.json").exists(),
            "a preview must not write inventory state"
        );

        run_ingest_pipeline(&bundle_dir, &request).unwrap();
        let after = preview_ingest_sources(&bundle_dir, &request).unwrap();
        assert!(after["files"].as_array().unwrap().iter().all(|f| f["classification"] == "unchanged_version"));
    }

    #[test]
    fn preview_on_a_missing_source_dir_is_the_same_error_the_pipeline_gives() {
        let (_dir, bundle_dir) = fresh_bundle();
        let request = base_request(Path::new("this/does/not/exist").to_path_buf());
        let err = preview_ingest_sources(&bundle_dir, &request).unwrap_err();
        assert!(err.to_string().starts_with("source directory does not exist:"));
    }

    /// The workbench-level retire entry point: retires, resyncs, and is inert on repeat.
    #[test]
    fn retire_bundle_source_retires_resyncs_and_is_inert_on_repeat() {
        let (_dir, bundle_dir) = fresh_bundle();
        let mut request = base_request(reference_source_dir());
        request.scan = true;
        request.normalize = true;
        request.mine = true;
        request.validate = true;
        run_ingest_pipeline(&bundle_dir, &request).unwrap();

        let inventory = sopkb_core::store::read_state_json(&bundle_dir, "inventory.json", Value::Null).unwrap();
        let target = inventory["sources"][0]["id"].as_str().unwrap().to_string();

        let outcome = retire_bundle_source(&bundle_dir, &target, "test:user", "out of scope").unwrap();
        assert!(outcome.did_change());

        let after = sopkb_core::store::read_state_json(&bundle_dir, "inventory.json", Value::Null).unwrap();
        assert_eq!(after["sources"][0]["status"], "retired");
        // The retirement is visible in the regenerated OKF document, proving the resync ran.
        let doc = std::fs::read_to_string(bundle_dir.join(format!("sources/{target}.md"))).unwrap();
        assert!(doc.contains("lifecycle_status: retired"), "{doc}");

        // Repeat: inert, and the OKF tree is not even rewritten.
        let doc_mtime = std::fs::metadata(bundle_dir.join(format!("sources/{target}.md"))).unwrap().modified().unwrap();
        let repeat = retire_bundle_source(&bundle_dir, &target, "test:user", "again").unwrap();
        assert!(!repeat.did_change());
        assert_eq!(
            std::fs::metadata(bundle_dir.join(format!("sources/{target}.md"))).unwrap().modified().unwrap(),
            doc_mtime,
            "a repeated retire must skip the resync entirely"
        );

        let (errors, _) = sopkb_review::validate_bundle(&bundle_dir).unwrap();
        assert_eq!(errors, Vec::<String>::new(), "a retired source must leave the bundle valid");
    }

    #[test]
    #[serial_test::serial(sopkb_settings_path_env)]
    fn azure_llm_provider_selection_skips_every_section_cleanly_without_configured_profile_not_a_panic() {
        // No SOPKB_SETTINGS_PATH override / no profile configured: every section's
        // azure-llm author call fails at config-resolution time ("Missing a
        // model/deployment name..."), before any network call is ever attempted.
        //
        // This used to abort the WHOLE run with an `Err` here, but that only ever
        // reflected `mine_with_author`'s OLD, pre-parallelization, non-Python-matching
        // shape. The real reference implementation (`okf_author.py`'s `call_author`,
        // see git show origin/integration/oss-launch:tools/sopkb/sopkb/okf_author.py
        // ~L272-343) has always caught every exception -- including a missing-config
        // one -- PER SECTION, logged it, and moved on; a config problem is not treated
        // as meaningfully different from a single section's LLM response being
        // malformed. Rust's `mine_with_author` (crates/sopkb-mining/src/okf_author.rs)
        // now matches that: config resolution happens inside each section's own
        // attempt loop, so a missing profile makes every section fail its 3 retries
        // and get skipped, not the run itself. This test now proves THAT: a clean,
        // itemless `Ok`, not a panic and not a whole-run `Err`.
        //
        // Isolate this process's view of settings so it can't accidentally pick up a
        // real profile from this machine's actual ~/.sopkb/settings.json.
        // `SOPKB_SETTINGS_PATH` is process-global, so this test is `#[serial]` -- same
        // precedent as `sopkb-llm`/`sopkb-mining`'s own env-var-mutating tests.
        let settings_dir = tempdir().unwrap();
        let settings_path = settings_dir.path().join("nonexistent-settings.json");
        unsafe {
            std::env::set_var("SOPKB_SETTINGS_PATH", &settings_path);
        }

        let (_dir, bundle_dir) = fresh_bundle();
        let mut request = base_request(reference_source_dir());
        request.scan = true;
        request.normalize = true;
        request.mine = true;
        request.mine_provider = "azure-llm".to_string();

        let result = run_ingest_pipeline(&bundle_dir, &request);

        unsafe {
            std::env::remove_var("SOPKB_SETTINGS_PATH");
        }

        let outcome = result.unwrap();
        // Not a panic (reaching this line already proves that), and every section
        // failed cleanly and was skipped rather than crashing or silently fabricating
        // items -- scan and normalize, which don't need a configured profile, still
        // ran and produced real output.
        assert!(outcome.sections.unwrap() > 0, "normalize must still run and find real sections");
        assert_eq!(outcome.items, Some(0), "every section's author call must fail config-resolution and be skipped, not crash or fabricate items");
        assert!(bundle_dir.join(".sopkb").join("inventory.json").exists(), "scan's writes must survive mine's per-section failures");
        assert!(bundle_dir.join(".sopkb").join("sections.json").exists(), "normalize's writes must survive mine's per-section failures");
    }
}
