//! `kl4a-core`: Rust port of `kl4a/kl4a/*.py`, the shared package sibling to
//! both `kl4a/codekb/` and `kl4a/apikb/` (already ported as the `codekb` and
//! `apikb` crates in this workspace). Multiple port batches independently
//! discovered call-outs to this package with no home in the Rust workspace
//! (`bundle_store`, `ids`, `okf_writer`, `hashing`, `llm_provider`,
//! `llm_settings`) and vendored rough approximations locally
//! (`codekb::kl4a_shared`) rather than block on it. This crate is that home;
//! `codekb::kl4a_shared` has since been folded in here and deleted.

pub mod bundle_store;
pub mod hashing;
pub mod ids;
pub mod llm_provider;
pub mod llm_settings;
pub mod okf_writer;
