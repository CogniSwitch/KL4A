//! Port of `mine_bundle` (`tools/sopkb/sopkb/mine.py`), this crate's top-level
//! provider-dispatch entry point. docs/port/port-mapping-b-mining-settings.md §3.1.

use crate::mine_fixture::mine_fixture_bundle;
use crate::okf_author::{azure_llm_author, mine_with_author};
use serde_json::Value;
use sopkb_core::error::{Result, SopkbError};
use std::path::Path;

/// Exactly three accepted provider strings: `"azure-llm"`, `"llm"`, `"fixture"` --
/// anything else (including `""`) raises the exact `ValueError` text below (P-M9
/// PRESERVE). `profile_id` is ignored on the fixture path.
///
/// **Default provider is `"azure-llm"`** per docs/port/DECISIONS.md Q6a -- this
/// OVERRIDES PORT_PLAN.md's "fixture" recommendation. There is no Rust-level default
/// parameter (unlike Python's `provider: str = "fixture"`); callers that want the
/// product's actual default must pass `"azure-llm"` explicitly. The fixture miner
/// itself is unaffected by which provider is the default -- it's fully implemented and
/// byte-exact regardless.
///
/// `on_progress`, when given, is forwarded to [`mine_with_author`] unchanged (see its
/// own doc comment) -- ignored on the fixture path, which has no network calls to
/// report progress on and already runs fast enough not to need it.
///
/// `is_cancelled`, when given, is likewise forwarded to [`mine_with_author`] unchanged
/// -- also ignored on the fixture path for the same reason (nothing slow enough there
/// to need cooperative cancellation).
pub fn mine_bundle(
    bundle_dir: &Path,
    provider: &str,
    profile_id: Option<&str>,
    on_progress: Option<&(dyn Fn(usize, usize) + Sync)>,
    is_cancelled: Option<&(dyn Fn() -> bool + Sync)>,
) -> Result<Vec<Value>> {
    if provider == "azure-llm" || provider == "llm" {
        // See sopkb_core::prompt_overrides's own doc comment -- a non-blank bundle-
        // level override wins over even a non-blank per-profile one.
        let overrides = sopkb_core::prompt_overrides::read_bundle_prompt_overrides(bundle_dir);
        let mining_override = if overrides.mining_prompt.trim().is_empty() { None } else { Some(overrides.mining_prompt) };
        return mine_with_author(
            bundle_dir,
            move |request| azure_llm_author(request, profile_id, mining_override.as_deref()),
            "sopkb/azure-llm",
            on_progress,
            is_cancelled,
        );
    }
    if provider != "fixture" {
        return Err(SopkbError::Value("provider must be fixture or azure-llm".to_string()));
    }
    mine_fixture_bundle(bundle_dir, "fixture")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_provider_strings_with_exact_message() {
        let dir = tempfile::tempdir().unwrap();
        let err = mine_bundle(dir.path(), "", None, None, None).unwrap_err();
        assert_eq!(err.to_string(), "provider must be fixture or azure-llm");
        let err = mine_bundle(dir.path(), "openai", None, None, None).unwrap_err();
        assert_eq!(err.to_string(), "provider must be fixture or azure-llm");
    }

    #[test]
    fn fixture_provider_runs_without_touching_llm_settings() {
        let dir = tempfile::tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        sopkb_core::store::create_bundle(&bundle_dir, Some("Empty")).unwrap();
        let items = mine_bundle(&bundle_dir, "fixture", None, None, None).unwrap();
        assert!(items.is_empty());
        assert!(bundle_dir.join(".sopkb").join("items.json").exists());
        assert!(bundle_dir.join(".sopkb").join("document_contexts.json").exists());
    }
}
