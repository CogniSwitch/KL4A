//! Global settings: ~/.sopkb/settings.json, profiles, env overrides. See docs/port/PORT_PLAN.md §3.1 (sopkb-config) and §6.7 (Phase 6).

pub mod profile;
pub mod settings;

pub use profile::{ModelProfile, PROFILE_FIELDS};
pub use settings::{
    auth_headers, delete_profile, get_default_profile_id, get_profile, list_profiles, load_config, max_parallel_workers, require,
    resolve, save_config, save_profile, set_default_profile, set_max_parallel_workers, set_reviewer_name, settings_path,
    unique_profile_id, reviewer_name, SettingsConfig, DEFAULT_MAX_PARALLEL_WORKERS,
};
