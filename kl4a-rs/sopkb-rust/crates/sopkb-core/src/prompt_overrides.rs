//! Per-BUNDLE prompt overrides (`.sopkb/prompt_overrides.json`). No Python
//! equivalent -- a genuinely new capability, not a port. Distinct from the
//! existing per-PROFILE `mining_prompt`/`chat_prompt` override
//! (`sopkb-config::ModelProfile`), which is global (follows the profile
//! everywhere it's used, across every bundle); this lets ONE bundle -- e.g. a
//! specialized insurance-policy bundle needing an author prompt tuned to that
//! domain's vocabulary -- override the prompt without touching a profile every
//! other bundle also uses.
//!
//! Precedence, applied by the mining/agent call sites that resolve a bundle_dir
//! (not here -- this module only reads/writes the file): a non-blank bundle
//! override wins over a non-blank profile override, which wins over the built-in
//! default. Same "non-blank string fully replaces, never merged" semantics as the
//! per-profile override (P-M18/P-A28), for consistency -- two different partial-
//! override semantics for two layers of the same concept would be confusing.

use crate::error::Result;
use crate::store;
use std::path::Path;

const STATE_FILENAME: &str = "prompt_overrides.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BundlePromptOverrides {
    #[serde(default)]
    pub mining_prompt: String,
    #[serde(default)]
    pub chat_prompt: String,
}

/// Missing file, or a present-but-malformed one, both read as "no overrides" --
/// this is a nice-to-have layered on top of an otherwise-working pipeline, and a
/// bundle with a slightly corrupt override file should still mine/chat using the
/// profile/default prompt rather than fail outright.
pub fn read_bundle_prompt_overrides(bundle_dir: &Path) -> BundlePromptOverrides {
    store::read_state_json(bundle_dir, STATE_FILENAME, serde_json::json!({}))
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

pub fn write_bundle_prompt_overrides(bundle_dir: &Path, overrides: &BundlePromptOverrides) -> Result<()> {
    store::write_state_json(bundle_dir, STATE_FILENAME, &serde_json::to_value(overrides).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_file_reads_as_blank_overrides() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        store::create_bundle(&bundle_dir, None).unwrap();
        assert_eq!(read_bundle_prompt_overrides(&bundle_dir), BundlePromptOverrides::default());
    }

    #[test]
    fn round_trips_a_written_override() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        store::create_bundle(&bundle_dir, None).unwrap();
        let overrides = BundlePromptOverrides { mining_prompt: "Custom mining prompt.".to_string(), chat_prompt: String::new() };
        write_bundle_prompt_overrides(&bundle_dir, &overrides).unwrap();
        assert_eq!(read_bundle_prompt_overrides(&bundle_dir), overrides);
    }

    #[test]
    fn malformed_file_reads_as_blank_overrides_rather_than_failing() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        store::create_bundle(&bundle_dir, None).unwrap();
        std::fs::write(bundle_dir.join(".sopkb").join(STATE_FILENAME), "not valid json").unwrap();
        assert_eq!(read_bundle_prompt_overrides(&bundle_dir), BundlePromptOverrides::default());
    }
}
