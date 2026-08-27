//! `sopkb-cli`: the user-facing CLI for the SOP Knowledge Bundle workbench. Ported from
//! `tools/sopkb/sopkb/cli.py::main`. See docs/port/PORT_PLAN.md §6.12 and
//! docs/port/port-mapping-e-web-cli-contract.md §5 for the subcommand table this mirrors.
//!
//! Contract (see `execute`/`main.rs`): every subcommand prints machine-parseable output on
//! success (JSON via `sopkb_fmt::to_canonical_json`, except the handful of markdown-report
//! commands noted per-arm below, which print the report text raw) and exits 0 -- except
//! `validate`, which exits 1 when `errors` is non-empty even though it does not error.
//! Any failure (a `sopkb_core::error::SopkbError` bubbling out of a backend call) is
//! printed as `error: {message}` to stderr and exits 1.
//!
//! This crate is split into a library (`sopkb_cli`, this file + `cli.rs`) and a thin `bin`
//! (`src/main.rs`) specifically so `execute`/`Cli::try_parse_from` are unit-testable
//! without spawning a subprocess -- see `tests/` for the integration coverage and
//! `v2/sopkb-rust/fixtures/harness/harness.py` for the differential-test harness that drives
//! the compiled binary directly against fixture `script` files.

pub mod cli;

use cli::{
    AgentCommand, BundleCommand, CitationsCommand, Cli, Command, ConflictsCommand, EvidenceCommand, FreshnessCommand,
    KnowledgeCommand, RelationsCommand, ReviewActionArgs, ReviewCommand, ReviewEditArgs, SectionsCommand, SourcesCommand,
};
use serde_json::Value;
use sopkb_core::error::{Result, SopkbError};
use sopkb_core::store::CreateBundleResult;
use std::path::{Path, PathBuf};

/// A command's successful outcome: text ready to print verbatim to stdout, plus the
/// process exit code to use (almost always 0 -- `validate` is the one exception).
#[derive(Debug)]
pub struct Success {
    pub text: String,
    pub exit_code: i32,
}

impl Success {
    fn ok(text: String) -> Self {
        Self { text, exit_code: 0 }
    }
}

fn json_val(value: Value) -> Result<Success> {
    Ok(Success::ok(sopkb_fmt::to_canonical_json(&value)))
}

fn json_list(values: Vec<Value>) -> Result<Success> {
    Ok(Success::ok(sopkb_fmt::to_canonical_json(&Value::Array(values))))
}

/// `create_bundle`'s two outcomes both carry an `id`, just via different shapes
/// (`CreateBundleResult::Created(Manifest).id` vs. the raw loaded YAML mapping's `"id"`
/// key on the idempotent-reinit path) -- extract it uniformly the way `cli.py`'s
/// `manifest['id'] if isinstance(manifest, dict) else manifest.id` does.
fn manifest_id(result: &CreateBundleResult) -> String {
    match result {
        CreateBundleResult::Created(manifest) => manifest.id.clone(),
        CreateBundleResult::Existing(map) => map.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    }
}

/// Reviewer default resolution -- a deliberate improvement over the original Python
/// CLI's behavior (see DEVIATIONS.md). The original CLI hardcodes `--reviewer` to the
/// literal `"local:user"` and never consults `settings.reviewer_name()`, while the web
/// path (`handle_review_form`) prefers the configured setting:
/// `form value or settings.reviewer_name() or "local:user"`. This reconciles the two: an
/// omitted (or blank) `--reviewer` resolves through the same fallback chain the web path
/// already uses, so the two entry points agree on who "the reviewer" is by default.
fn resolve_reviewer(explicit: Option<String>) -> String {
    if let Some(reviewer) = explicit {
        if !reviewer.is_empty() {
            return reviewer;
        }
    }
    let configured = sopkb_config::reviewer_name();
    if !configured.trim().is_empty() {
        configured
    } else {
        "local:user".to_string()
    }
}

fn review_action(
    args: ReviewActionArgs,
    action: fn(&Path, &str, &str, &str) -> Result<Value>,
) -> Result<Success> {
    let bundle_dir = PathBuf::from(args.bundle_dir);
    let reviewer = resolve_reviewer(args.reviewer);
    let value = action(&bundle_dir, &args.knowledge_item_id, &reviewer, &args.rationale)?;
    json_val(value)
}

/// Runs the parsed command against the backend crates and produces the text to print
/// plus the process exit code. Backend errors propagate as `Err` -- `main.rs` is
/// responsible for the `error: {e}` / exit-1 half of the contract described above.
pub fn execute(command: Command) -> Result<Success> {
    match command {
        Command::Init { bundle_dir, title } => {
            let bundle_dir = PathBuf::from(bundle_dir);
            let result = sopkb_core::store::create_bundle(&bundle_dir, title.as_deref())?;
            sopkb_export::sync_okf_bundle(&bundle_dir)?;
            Ok(Success::ok(format!("Initialized bundle {}", manifest_id(&result))))
        }
        Command::Scan { source_dir, bundle } => {
            let source_dir = PathBuf::from(source_dir);
            let bundle_dir = PathBuf::from(bundle);
            let sources = sopkb_core::inventory::scan_sources(&source_dir, &bundle_dir)?;
            sopkb_export::sync_okf_bundle(&bundle_dir)?;
            Ok(Success::ok(format!("Scanned {} source(s)", sources.len())))
        }
        Command::Normalize { bundle_dir, provider, profile } => {
            let bundle_dir = PathBuf::from(bundle_dir);
            let log_warning = |message: &str| sopkb_core::store::append_ingest_log(&bundle_dir, message);
            let restructure = sopkb_workbench::provider_hook(&provider, profile.as_deref(), None, None, Some(&log_warning));
            let sections = sopkb_core::normalize::normalize_sources(&bundle_dir, restructure.as_deref(), Some(sopkb_config::max_parallel_workers()))?;
            sopkb_export::sync_okf_bundle(&bundle_dir)?;
            Ok(Success::ok(format!("Normalized {} section(s)", sections.len())))
        }
        Command::Validate { bundle_dir } => {
            let bundle_dir = PathBuf::from(bundle_dir);
            let (errors, warnings) = sopkb_review::validate_bundle(&bundle_dir)?;
            let exit_code = if errors.is_empty() { 0 } else { 1 };
            let text = format!("Validation completed with {} error(s), {} warning(s)", errors.len(), warnings.len());
            Ok(Success { text, exit_code })
        }
        Command::Mine { bundle_dir, provider, profile } => {
            let bundle_dir = PathBuf::from(bundle_dir);
            let items = sopkb_mining::mine_bundle(&bundle_dir, &provider, profile.as_deref(), None, None)?;
            sopkb_export::sync_okf_bundle(&bundle_dir)?;
            Ok(Success::ok(format!("Mined {} proposed knowledge item(s)", items.len())))
        }
        Command::Export { bundle_dir, format } => {
            let bundle_dir = PathBuf::from(bundle_dir);
            let formats: Vec<String> = format.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let exports = sopkb_export::export_bundle(&bundle_dir, &formats)?;
            Ok(Success::ok(format!("Generated {} export(s)", exports.len())))
        }

        Command::Bundle { command: BundleCommand::Describe { bundle_dir } } => {
            json_val(sopkb_derive::reads::bundle_describe(&PathBuf::from(bundle_dir))?)
        }

        Command::Sources { command } => match command {
            SourcesCommand::List { bundle_dir } => json_list(sopkb_derive::reads::sources_list(&PathBuf::from(bundle_dir))?),
            SourcesCommand::Get { bundle_dir, source_id } => {
                json_val(sopkb_derive::reads::sources_get(&PathBuf::from(bundle_dir), &source_id)?)
            }
            SourcesCommand::Retire { bundle_dir, source_id, rationale, actor } => {
                let bundle_dir = PathBuf::from(bundle_dir);
                // Under the same bundle lock every other mutating operation takes --
                // retirement rewrites four state files and the manifest, so a concurrent
                // ingest would otherwise be free to interleave with it.
                let outcome = sopkb_review::with_bundle_lock(&bundle_dir, || {
                    sopkb_review::retire_source(&bundle_dir, &source_id, &actor, &rationale)
                })?;
                sopkb_export::sync_okf_bundle(&bundle_dir)?;
                match outcome {
                    sopkb_review::RetireOutcome::Retired(_) => {
                        Ok(Success::ok(format!("Retired source {source_id}")))
                    }
                    sopkb_review::RetireOutcome::AlreadyRetired(_) => {
                        Ok(Success::ok(format!("Source {source_id} is already retired; nothing changed")))
                    }
                }
            }
            SourcesCommand::Preview { source_dir, bundle } => json_val(
                sopkb_core::inventory::classify_source_updates(&PathBuf::from(source_dir), &PathBuf::from(bundle))?,
            ),
        },

        Command::Sections { command } => match command {
            SectionsCommand::List { bundle_dir } => json_list(sopkb_derive::reads::sections_list(&PathBuf::from(bundle_dir))?),
            SectionsCommand::Get { bundle_dir, section_id } => {
                json_val(sopkb_derive::reads::sections_get(&PathBuf::from(bundle_dir), &section_id)?)
            }
        },

        Command::Knowledge { command } => match command {
            KnowledgeCommand::Search { bundle_dir, query } => {
                json_list(sopkb_derive::reads::knowledge_search(&PathBuf::from(bundle_dir), &query)?)
            }
            KnowledgeCommand::Get { bundle_dir, knowledge_item_id } => {
                json_val(sopkb_derive::reads::knowledge_get(&PathBuf::from(bundle_dir), &knowledge_item_id)?)
            }
        },

        Command::Evidence { command: EvidenceCommand::Get { bundle_dir, knowledge_item_id } } => {
            json_val(sopkb_derive::reads::evidence_get(&PathBuf::from(bundle_dir), &knowledge_item_id)?)
        }

        Command::Conflicts { command: ConflictsCommand::List { bundle_dir } } => {
            Ok(Success::ok(sopkb_derive::reads::conflicts_list(&PathBuf::from(bundle_dir))))
        }

        Command::Freshness { command: FreshnessCommand::Check { bundle_dir } } => {
            Ok(Success::ok(sopkb_derive::reads::freshness_check(&PathBuf::from(bundle_dir))))
        }

        Command::Citations { command: CitationsCommand::Resolve { bundle_dir, citation_id } } => {
            json_val(sopkb_derive::reads::citations_resolve(&PathBuf::from(bundle_dir), &citation_id)?)
        }

        Command::Agent { command } => match command {
            AgentCommand::Guide { bundle_dir } => Ok(Success::ok(sopkb_derive::context::agent_guide(&PathBuf::from(bundle_dir)))),
            AgentCommand::Tasks { bundle_dir } => json_list(sopkb_derive::context::agent_tasks(&PathBuf::from(bundle_dir))?),
            AgentCommand::Context { bundle_dir, task, include_rejected } => {
                json_val(sopkb_derive::context::agent_context(&PathBuf::from(bundle_dir), &task, include_rejected)?)
            }
            AgentCommand::Chat { bundle_dir, scenario, task, provider, profile, allow_proposed } => {
                let value = sopkb_agent::handle_agent_chat(
                    &PathBuf::from(bundle_dir),
                    &task,
                    &scenario,
                    allow_proposed,
                    &provider,
                    profile.as_deref(),
                )?;
                json_val(value)
            }
        },

        Command::Relations { command } => match command {
            RelationsCommand::Search { bundle_dir, subject, predicate, object } => json_list(sopkb_derive::relations::relations_search(
                &PathBuf::from(bundle_dir),
                &subject,
                &predicate,
                &object,
            )?),
            RelationsCommand::Neighborhood { bundle_dir, node_id } => {
                json_val(sopkb_derive::relations::relations_neighborhood(&PathBuf::from(bundle_dir), &node_id)?)
            }
        },

        Command::Review { command } => match command {
            ReviewCommand::Approve(args) => review_action(args, sopkb_review::approve_item),
            ReviewCommand::Reject(args) => review_action(args, sopkb_review::reject_item),
            ReviewCommand::Defer(args) => review_action(args, sopkb_review::defer_item),
            ReviewCommand::Comment(args) => review_action(args, sopkb_review::comment_item),
            ReviewCommand::Edit(ReviewEditArgs { bundle_dir, knowledge_item_id, field, value, rationale, reviewer }) => {
                let bundle_dir = PathBuf::from(bundle_dir);
                let reviewer = resolve_reviewer(reviewer);
                json_val(sopkb_review::edit_item(&bundle_dir, &knowledge_item_id, &field, &value, &reviewer, &rationale)?)
            }
        },
    }
}

/// Convenience wrapper for tests/integration use: parse argv (argv\[0\] excluded, i.e. the
/// slice `clap` expects after `Cli::try_parse_from`'s own leading-arg convention) and run
/// it. Returns a clap usage/parse error as `Err(String)` distinctly from a backend
/// `SopkbError`, since clap errors are not backend failures.
pub fn run<I, T>(args: I) -> std::result::Result<Success, RunError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    use clap::Parser;
    let cli = Cli::try_parse_from(args).map_err(RunError::Usage)?;
    execute(cli.command).map_err(RunError::Backend)
}

#[derive(Debug)]
pub enum RunError {
    /// A clap usage/parse error (missing required argument, unknown subcommand, etc.).
    Usage(clap::Error),
    /// A backend error from one of the crates this CLI calls into.
    Backend(SopkbError),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Usage(e) => write!(f, "{e}"),
            RunError::Backend(e) => write!(f, "error: {e}"),
        }
    }
}
