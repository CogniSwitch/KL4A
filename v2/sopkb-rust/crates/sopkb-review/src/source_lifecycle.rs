//! Port of `tools/sopkb/sopkb/source_lifecycle.py` -- the retire-source operation.
//!
//! # Why this lives in `sopkb-review` rather than `sopkb-core`
//!
//! Retiring a source ends with a full `validate_bundle` pass, and `validate_bundle`
//! lives here. The Python original dodges the resulting import cycle with a
//! function-local `from .validate import validate_bundle`; Rust has no equivalent
//! escape hatch, since a crate cycle is a hard error. So the *decision* half of the
//! operation (which records change, and how) sits in
//! [`sopkb_core::lifecycle::plan_retire_source`], pure and dependency-free, and only
//! the write-and-validate half lives here, one crate up. That split is what makes the
//! state machine unit-testable without a bundle on disk at all.
//!
//! # The two defects this port fixes rather than carries over
//!
//! Both were found in the 2026-08-21 code review of the Python original and are named
//! in `docs/port/CATCHUP_PLAN.md`:
//!
//! 1. **Non-transactional.** The original writes `inventory.json`, `items.json`,
//!    `source_versions.json`, `source_events.json` and `manifest.yaml` one after
//!    another and only *then* validates. A failure part-way through -- or a validation
//!    pass that raises afterwards -- leaves the bundle permanently half-mutated, with
//!    no rollback and no record of how far it got.
//! 2. **Non-idempotent.** Retiring an already-retired source appends a second,
//!    near-identical `source_retired` event whose `previous_value.status` is
//!    `"retired"`, permanently polluting an append-only audit trail with a
//!    state-transition that never happened.

use serde_json::{json, Value};
use sopkb_core::error::{Result, SopkbError};
use sopkb_core::lifecycle::{
    self, FileTransaction, RetirePlan, SOURCE_EVENTS_FILE, SOURCE_VERSIONS_FILE,
};
use sopkb_core::store;
use std::path::Path;

/// The default `actor` recorded on a retirement event, matching the reference
/// implementation's keyword default.
pub const DEFAULT_ACTOR: &str = "local:user";

/// What a retire call did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetireOutcome {
    /// The source was active and has now been retired. Carries the appended event.
    Retired(Value),
    /// The source was already retired; nothing was written. Carries the *original*
    /// retirement event when the log still has it, so a caller that re-issues the
    /// request gets a coherent answer rather than a synthesized one.
    AlreadyRetired(Option<Value>),
}

impl RetireOutcome {
    /// The event to report to a caller that just wants "the retirement event", whether
    /// this call performed the retirement or an earlier one did.
    pub fn event(&self) -> Option<&Value> {
        match self {
            RetireOutcome::Retired(event) => Some(event),
            RetireOutcome::AlreadyRetired(event) => event.as_ref(),
        }
    }

    pub fn did_change(&self) -> bool {
        matches!(self, RetireOutcome::Retired(_))
    }
}

/// Retires `source_id`: marks the source and its active versions `retired`, flips its
/// active knowledge items to `lifecycle_status: "retired"`, appends one
/// `source_retired` event, and rewrites the manifest -- all or nothing.
///
/// Idempotent: retiring an already-retired source writes nothing at all and returns
/// [`RetireOutcome::AlreadyRetired`].
///
/// Nothing is deleted. Evidence, spans, and the normalized text all survive; the
/// retired knowledge simply stops appearing in the default agent context.
pub fn retire_source(bundle_dir: &Path, source_id: &str, actor: &str, rationale: &str) -> Result<RetireOutcome> {
    retire_source_with_validator(bundle_dir, source_id, actor, rationale, |dir| {
        crate::validate::validate_bundle(dir).map(|_| ())
    })
}

/// [`retire_source`] with the post-write validation pass injected, so tests can prove
/// the rollback path with a validator that fails on demand. Production callers want
/// [`retire_source`].
///
/// Sequence, and why it is this way:
///
/// 1. Migrate, so a legacy bundle has the version metadata this operates on.
/// 2. Read state and compute the ENTIRE new state in memory
///    ([`sopkb_core::lifecycle::plan_retire_source`]). Any error here -- unknown source
///    id, malformed inventory -- happens before a single byte is written.
/// 3. Open a [`FileTransaction`] and snapshot `reports/` up front, because the
///    validator regenerates those files and they must be undoable too.
/// 4. Write the five files.
/// 5. Validate. On failure, roll everything back and return the error; then re-run the
///    validator best-effort so derived artifacts match the restored state again.
pub fn retire_source_with_validator<V>(
    bundle_dir: &Path,
    source_id: &str,
    actor: &str,
    rationale: &str,
    validator: V,
) -> Result<RetireOutcome>
where
    V: Fn(&Path) -> Result<()>,
{
    lifecycle::migrate_source_version_state(bundle_dir)?;

    let inventory = store::read_state_json(bundle_dir, "inventory.json", json!({"sources": []}))?;
    let items = store::read_state_json(bundle_dir, "items.json", json!([]))?;
    let events = lifecycle::read_source_events(bundle_dir)?;

    let plan = lifecycle::plan_retire_source(
        &inventory,
        &items,
        &events,
        source_id,
        actor,
        rationale,
        &store::utc_now(),
    )?;

    let mutation = match plan {
        // The idempotency guard. Returning before opening a transaction means a
        // repeated retire is not merely harmless but genuinely inert: no write, no
        // duplicate audit event, and not even a manifest `updated_at` bump.
        RetirePlan::AlreadyRetired { existing_event } => return Ok(RetireOutcome::AlreadyRetired(existing_event)),
        RetirePlan::Retire(mutation) => mutation,
    };

    let mut transaction = FileTransaction::new();
    // The validator rewrites `reports/*` and the OKF documents derived from bundle
    // state. `reports/` is snapshotted so a rollback restores those too; the OKF
    // document tree is not, and is regenerated by the best-effort re-validate below.
    transaction.snapshot_dir_files(&bundle_dir.join("reports"))?;

    transaction.write_state_json(bundle_dir, "inventory.json", &mutation.inventory)?;
    transaction.write_state_json(bundle_dir, "items.json", &mutation.items)?;
    transaction.write_state_json(bundle_dir, SOURCE_VERSIONS_FILE, &mutation.source_versions)?;
    transaction.write_state_json(bundle_dir, SOURCE_EVENTS_FILE, &mutation.events)?;
    lifecycle::update_manifest_sources(bundle_dir, &mutation.sources, Some(&mut transaction))?;

    match validator(bundle_dir) {
        Ok(()) => {
            transaction.commit();
            Ok(RetireOutcome::Retired(mutation.event))
        }
        Err(validation_error) => {
            transaction.rollback()?;
            // Derived artifacts (`reports/` beyond what we snapshotted, the OKF
            // document tree) may still reflect the attempted state. Regenerate them
            // from the restored state. Best effort by design: if the validator is
            // failing for a reason unrelated to this retirement it will fail again,
            // and the caller needs to see the ORIGINAL error, not this one.
            let _ = validator(bundle_dir);
            Err(SopkbError::Value(format!(
                "retire_source rolled back: validation failed after writing state: {validation_error}"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    /// A real ingested bundle: one markdown source, normalized, mined with the
    /// deterministic fixture provider. Same setup as the reference implementation's
    /// own `test_source_retirement.py`.
    fn ingested_bundle() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        let source_dir = dir.path().join("sources");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("policy.md"), "# Eligibility\n\nStaff must confirm patient identity.\n").unwrap();
        sopkb_core::store::create_bundle(&bundle_dir, Some("Retire Bundle")).unwrap();
        sopkb_core::inventory::scan_sources(&source_dir, &bundle_dir).unwrap();
        sopkb_core::normalize::normalize_sources(&bundle_dir, None, None).unwrap();
        sopkb_mining::mine_bundle(&bundle_dir, "fixture", None, None, None).unwrap();
        (dir, bundle_dir)
    }

    fn read(bundle_dir: &Path, filename: &str) -> Value {
        store::read_state_json(bundle_dir, filename, Value::Null).unwrap()
    }

    /// The happy path, mirroring `test_source_retirement.py`'s assertions.
    #[test]
    fn retiring_a_source_marks_source_versions_and_items_and_preserves_evidence() {
        let (_dir, bundle_dir) = ingested_bundle();
        let mined = read(&bundle_dir, "items.json");
        let mined_id = mined[0]["id"].as_str().unwrap().to_string();

        let outcome = retire_source(&bundle_dir, "policy", "test:user", "Removed from active SOP scope.").unwrap();
        assert!(outcome.did_change());
        assert_eq!(outcome.event().unwrap()["action"], "source_retired");
        assert_eq!(outcome.event().unwrap()["rationale"], "Removed from active SOP scope.");
        assert_eq!(outcome.event().unwrap()["actor"], "test:user");

        let inventory = read(&bundle_dir, "inventory.json");
        assert_eq!(inventory["sources"][0]["status"], "retired");
        let registry = read(&bundle_dir, SOURCE_VERSIONS_FILE);
        assert_eq!(registry["versions"][0]["status"], "retired");
        let items = read(&bundle_dir, "items.json");
        assert_eq!(items[0]["id"], mined_id.as_str());
        assert_eq!(items[0]["lifecycle_status"], "retired");

        // Nothing deleted: the normalized text and the original both survive.
        assert!(bundle_dir.join("sources/originals/policy__v1.md").exists());
        assert!(bundle_dir.join("sources/normalized/policy__v1.md").exists());

        let impact = fs::read_to_string(bundle_dir.join("reports/source_update_impact.md")).unwrap();
        assert!(impact.contains("Retired source versions: 1"), "impact report:\n{impact}");
        assert!(impact.contains("Retired knowledge items: 1"), "impact report:\n{impact}");

        // The manifest tracks the new status too.
        let manifest = store::load_manifest(&bundle_dir).unwrap();
        let entry = manifest.get("sources").unwrap().as_sequence().unwrap()[0].clone();
        assert_eq!(entry.as_mapping().unwrap().get("status").unwrap().as_str(), Some("retired"));
    }

    #[test]
    fn retiring_leaves_the_bundle_valid() {
        let (_dir, bundle_dir) = ingested_bundle();
        retire_source(&bundle_dir, "policy", DEFAULT_ACTOR, "no longer in scope").unwrap();
        let (errors, _warnings) = crate::validate::validate_bundle(&bundle_dir).unwrap();
        assert_eq!(errors, Vec::<String>::new());
    }

    /// CATCHUP_PLAN.md fix #1 (idempotency): the reference implementation appends a
    /// second `source_retired` event here, with `previous_value.status == "retired"` --
    /// an audit record of a transition that never happened.
    #[test]
    fn retiring_twice_writes_nothing_the_second_time_and_never_duplicates_the_audit_event() {
        let (_dir, bundle_dir) = ingested_bundle();
        let first = retire_source(&bundle_dir, "policy", "test:user", "first").unwrap();
        assert!(first.did_change());

        let events_after_first = read(&bundle_dir, SOURCE_EVENTS_FILE);
        let manifest_after_first = fs::read(bundle_dir.join("manifest.yaml")).unwrap();
        let inventory_after_first = fs::read(store::state_path(&bundle_dir, "inventory.json")).unwrap();

        let second = retire_source(&bundle_dir, "policy", "test:user", "second, with a different rationale").unwrap();
        assert!(!second.did_change(), "a second retire must report that it changed nothing");
        assert_eq!(second, RetireOutcome::AlreadyRetired(first.event().cloned()));

        let events_after_second = read(&bundle_dir, SOURCE_EVENTS_FILE);
        assert_eq!(events_after_second, events_after_first, "the audit log must not gain a duplicate event");
        let retired_events = events_after_second
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["action"] == "source_retired")
            .count();
        assert_eq!(retired_events, 1);

        // Not merely "no duplicate event" -- genuinely inert, down to `updated_at`.
        assert_eq!(fs::read(bundle_dir.join("manifest.yaml")).unwrap(), manifest_after_first);
        assert_eq!(fs::read(store::state_path(&bundle_dir, "inventory.json")).unwrap(), inventory_after_first);
    }

    /// CATCHUP_PLAN.md fix #2 (transactionality), by injection: a validator that fails
    /// AFTER the five files are written must leave the bundle byte-identical to how it
    /// started. Under the reference implementation this same scenario leaves the source
    /// retired, the items retired, and the event appended, with no way back.
    #[test]
    fn a_validation_failure_after_the_writes_rolls_every_file_back() {
        let (_dir, bundle_dir) = ingested_bundle();

        let before = snapshot_state(&bundle_dir);

        let err = retire_source_with_validator(&bundle_dir, "policy", "test:user", "why", |_| {
            Err(SopkbError::Value("injected validation failure".to_string()))
        })
        .unwrap_err();
        assert!(err.to_string().contains("injected validation failure"));
        assert!(err.to_string().contains("rolled back"));

        let after = snapshot_state(&bundle_dir);
        for (path, before_bytes) in &before {
            assert_eq!(
                after.get(path),
                Some(before_bytes),
                "{path} must be byte-identical after a rolled-back retire"
            );
        }
        assert_eq!(before.len(), after.len(), "the rollback must not leave extra files behind either");

        // And the state is semantically unchanged, not just byte-equal by luck.
        let inventory = read(&bundle_dir, "inventory.json");
        assert_eq!(inventory["sources"][0]["status"], "active");
        let items = read(&bundle_dir, "items.json");
        assert_eq!(items[0]["lifecycle_status"], "active");
        let events = read(&bundle_dir, SOURCE_EVENTS_FILE);
        assert!(
            !events.as_array().unwrap().iter().any(|e| e["action"] == "source_retired"),
            "a rolled-back retire must leave no trace in the audit log"
        );
    }

    /// A rolled-back retire must leave the bundle usable, not just unchanged: the very
    /// next retire has to succeed normally.
    #[test]
    fn a_bundle_recovers_completely_from_a_rolled_back_retire() {
        let (_dir, bundle_dir) = ingested_bundle();
        let _ = retire_source_with_validator(&bundle_dir, "policy", "u", "r", |_| {
            Err(SopkbError::Value("boom".to_string()))
        });
        let outcome = retire_source(&bundle_dir, "policy", "u", "second attempt").unwrap();
        assert!(outcome.did_change());
        assert_eq!(outcome.event().unwrap()["id"], "source-retired-policy-000002");
        let (errors, _) = crate::validate::validate_bundle(&bundle_dir).unwrap();
        assert_eq!(errors, Vec::<String>::new());
    }

    /// The rollback path is only reached if the writes actually happened first --
    /// otherwise the test above would pass trivially against a no-op implementation.
    #[test]
    fn the_rollback_test_is_not_vacuous_the_writes_do_land_before_validation_runs() {
        let (_dir, bundle_dir) = ingested_bundle();
        let saw_retired_state_on_disk = Cell::new(false);
        let _ = retire_source_with_validator(&bundle_dir, "policy", "u", "r", |dir| {
            let inventory = store::read_state_json(dir, "inventory.json", Value::Null).unwrap();
            if inventory["sources"][0]["status"] == "retired" {
                saw_retired_state_on_disk.set(true);
            }
            Err(SopkbError::Value("boom".to_string()))
        });
        assert!(
            saw_retired_state_on_disk.get(),
            "the validator must observe the fully-written retired state, or the rollback test proves nothing"
        );
    }

    #[test]
    fn retiring_an_unknown_source_is_an_error_and_writes_nothing() {
        let (_dir, bundle_dir) = ingested_bundle();
        let before = snapshot_state(&bundle_dir);
        let err = retire_source(&bundle_dir, "does-not-exist", DEFAULT_ACTOR, "r").unwrap_err();
        assert_eq!(err.to_string(), "source not found: does-not-exist");
        assert_eq!(snapshot_state(&bundle_dir), before);
    }

    /// Retirement only touches the named source's items.
    #[test]
    fn retiring_one_source_leaves_another_sources_knowledge_active() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        let source_dir = dir.path().join("sources");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("policy.md"), "# Eligibility\n\nStaff must confirm patient identity.\n").unwrap();
        fs::write(source_dir.join("handbook.md"), "# Escalation\n\nClinicians should escalate promptly.\n").unwrap();
        sopkb_core::store::create_bundle(&bundle_dir, Some("Two Sources")).unwrap();
        sopkb_core::inventory::scan_sources(&source_dir, &bundle_dir).unwrap();
        sopkb_core::normalize::normalize_sources(&bundle_dir, None, None).unwrap();
        sopkb_mining::mine_bundle(&bundle_dir, "fixture", None, None, None).unwrap();

        retire_source(&bundle_dir, "policy", DEFAULT_ACTOR, "r").unwrap();

        let items = read(&bundle_dir, "items.json");
        for item in items.as_array().unwrap() {
            let expected = if item["source_id"] == "policy" { "retired" } else { "active" };
            assert_eq!(item["lifecycle_status"], expected, "item {}", item["id"]);
        }
        let inventory = read(&bundle_dir, "inventory.json");
        for source in inventory["sources"].as_array().unwrap() {
            let expected = if source["id"] == "policy" { "retired" } else { "active" };
            assert_eq!(source["status"], expected);
        }
    }

    /// Every file the operation can touch, as raw bytes, for byte-exact before/after
    /// comparison. Deliberately includes files the transaction does NOT manage
    /// (`sections.json`) as a control.
    fn snapshot_state(bundle_dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
        let mut out = std::collections::BTreeMap::new();
        let mut record = |label: &str, path: PathBuf| {
            if let Ok(bytes) = fs::read(&path) {
                out.insert(label.to_string(), bytes);
            }
        };
        for name in ["inventory.json", "items.json", "sections.json", SOURCE_VERSIONS_FILE, SOURCE_EVENTS_FILE] {
            record(name, store::state_path(bundle_dir, name));
        }
        record("manifest.yaml", bundle_dir.join("manifest.yaml"));
        let reports = bundle_dir.join("reports");
        if let Ok(entries) = fs::read_dir(&reports) {
            for entry in entries.flatten().filter(|e| e.path().is_file()) {
                let label = format!("reports/{}", entry.file_name().to_string_lossy());
                record(&label, entry.path());
            }
        }
        out
    }
}
