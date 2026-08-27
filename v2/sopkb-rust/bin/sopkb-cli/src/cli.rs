//! Argument parsing for the user-facing `sopkb-cli` binary (clap derive). Mirrors
//! `tools/sopkb/sopkb/cli.py`'s `build_parser` subcommand shape (same subcommand names,
//! same positional/flag layout) so the fixture differential-test harness's `script` files
//! (`v2/sopkb-rust/fixtures/cases/*/script`, e.g. `mine {bundle} --provider fixture`) parse
//! identically against both engines. Two deliberate additions beyond the Python CLI, per
//! docs/port/port-mapping-e-web-cli-contract.md §5 and this port's DECISIONS.md:
//!   - `mine --profile <id>` (Python's CLI has no `--profile` flag at all).
//!   - `agent chat` (not in the original CLI; the only way to exercise `sopkb-agent` from
//!     the command line, following the same flag/JSON-output conventions as `agent context`).

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "sopkb-cli", about = "SOP Knowledge Bundle workbench", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create an SOP Knowledge Bundle
    Init {
        bundle_dir: String,
        #[arg(long)]
        title: Option<String>,
    },
    /// Inventory source documents
    Scan {
        source_dir: String,
        #[arg(long)]
        bundle: String,
    },
    /// Normalize inventoried sources
    Normalize {
        bundle_dir: String,
        /// Default is "fixture", matching Python's own `normalize_sources`
        /// default -- unlike `mine`, there's no DECISIONS.md entry calling for a
        /// different default here, so this stays unsurprising: normalizing
        /// without an explicit `--provider` never requires LLM credentials.
        #[arg(long, default_value = "fixture")]
        provider: String,
        /// Model profile id, same meaning as `mine --profile`.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Validate bundle structure (exit code 1 if any error is found)
    Validate { bundle_dir: String },
    /// Generate proposed knowledge
    Mine {
        bundle_dir: String,
        /// Default is "azure-llm" per docs/port/DECISIONS.md Q6a -- diverges from the
        /// original Python CLI's "fixture" default.
        #[arg(long, default_value = "azure-llm")]
        provider: String,
        /// Model profile id (new: the original Python CLI has no such flag).
        #[arg(long)]
        profile: Option<String>,
    },
    /// Export bundle artifacts
    Export {
        bundle_dir: String,
        #[arg(long, default_value = "graph-json,rdf")]
        format: String,
    },
    /// Agent bundle tools
    Bundle {
        #[command(subcommand)]
        command: BundleCommand,
    },
    /// Agent source tools
    Sources {
        #[command(subcommand)]
        command: SourcesCommand,
    },
    /// Agent section tools
    Sections {
        #[command(subcommand)]
        command: SectionsCommand,
    },
    /// Agent knowledge tools
    Knowledge {
        #[command(subcommand)]
        command: KnowledgeCommand,
    },
    /// Agent evidence tools
    Evidence {
        #[command(subcommand)]
        command: EvidenceCommand,
    },
    /// Agent conflict tools
    Conflicts {
        #[command(subcommand)]
        command: ConflictsCommand,
    },
    /// Agent freshness tools
    Freshness {
        #[command(subcommand)]
        command: FreshnessCommand,
    },
    /// Agent citation tools
    Citations {
        #[command(subcommand)]
        command: CitationsCommand,
    },
    /// Agent consumption layer
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    /// Knowledge Relation tools
    Relations {
        #[command(subcommand)]
        command: RelationsCommand,
    },
    /// Human review commands
    Review {
        #[command(subcommand)]
        command: ReviewCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum BundleCommand {
    /// Describe a bundle
    Describe { bundle_dir: String },
}

#[derive(Subcommand, Debug)]
pub enum SourcesCommand {
    /// List sources
    List { bundle_dir: String },
    /// Get a source by id
    Get { bundle_dir: String, source_id: String },
    /// Retire a source: excludes it and its knowledge from the active set without
    /// deleting anything. Idempotent -- retiring an already-retired source is a no-op.
    ///
    /// New in this port (the Python CLI has no equivalent; retirement is reachable only
    /// through its web UI), same category of deliberate addition as `mine --profile`.
    Retire {
        bundle_dir: String,
        source_id: String,
        /// Why this source is being retired. Required, and recorded verbatim in the
        /// append-only audit event -- a retirement with no stated reason is not worth
        /// having in an audit trail.
        #[arg(long)]
        rationale: String,
        #[arg(long, default_value = sopkb_review::DEFAULT_ACTOR)]
        actor: String,
    },
    /// Preview what a scan of SOURCE_DIR would do to the bundle -- which files are new
    /// sources, which are new versions of existing ones, and which are unchanged --
    /// without copying, writing, or versioning anything.
    Preview {
        source_dir: String,
        #[arg(long)]
        bundle: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum SectionsCommand {
    /// List sections
    List { bundle_dir: String },
    /// Get a section by id
    Get { bundle_dir: String, section_id: String },
}

#[derive(Subcommand, Debug)]
pub enum KnowledgeCommand {
    /// Search knowledge items
    Search { bundle_dir: String, query: String },
    /// Get a knowledge item by id
    Get { bundle_dir: String, knowledge_item_id: String },
}

#[derive(Subcommand, Debug)]
pub enum EvidenceCommand {
    /// Get evidence for a knowledge item
    Get { bundle_dir: String, knowledge_item_id: String },
}

#[derive(Subcommand, Debug)]
pub enum ConflictsCommand {
    /// List conflicts (raw markdown report)
    List { bundle_dir: String },
}

#[derive(Subcommand, Debug)]
pub enum FreshnessCommand {
    /// Check freshness (raw markdown report)
    Check { bundle_dir: String },
}

#[derive(Subcommand, Debug)]
pub enum CitationsCommand {
    /// Resolve a citation id to its evidence
    Resolve { bundle_dir: String, citation_id: String },
}

#[derive(Subcommand, Debug)]
pub enum AgentCommand {
    /// Read the agent guide (raw markdown)
    Guide { bundle_dir: String },
    /// List agent task contexts
    Tasks { bundle_dir: String },
    /// Get task-scoped agent context
    Context {
        bundle_dir: String,
        #[arg(long)]
        task: String,
        /// Only caller in this port (CLI/web/MCP) that can set include_rejected=true.
        #[arg(long)]
        include_rejected: bool,
    },
    /// Run a single-shot agent chat turn and persist it to the bundle's transcript
    Chat {
        bundle_dir: String,
        #[arg(long)]
        scenario: String,
        #[arg(long, default_value = "auto")]
        task: String,
        /// Default is "azure-llm" per docs/port/DECISIONS.md Q6b.
        #[arg(long, default_value = "azure-llm")]
        provider: String,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        allow_proposed: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum RelationsCommand {
    /// Search Knowledge Relations
    Search {
        bundle_dir: String,
        #[arg(long, default_value = "")]
        subject: String,
        #[arg(long, default_value = "")]
        predicate: String,
        #[arg(long = "object", default_value = "")]
        object: String,
    },
    /// Get a relation neighborhood
    Neighborhood { bundle_dir: String, node_id: String },
}

#[derive(Args, Debug)]
pub struct ReviewActionArgs {
    pub bundle_dir: String,
    pub knowledge_item_id: String,
    #[arg(long)]
    pub rationale: String,
    /// Omitted -> resolve via sopkb_config::reviewer_name(), falling back to
    /// "local:user" only if that is also blank (see DEVIATIONS.md).
    #[arg(long)]
    pub reviewer: Option<String>,
}

#[derive(Args, Debug)]
pub struct ReviewEditArgs {
    pub bundle_dir: String,
    pub knowledge_item_id: String,
    #[arg(long)]
    pub field: String,
    /// Always a plain string; `edit_item` does Python float()-grammar parsing
    /// internally for the `confidence` field.
    #[arg(long)]
    pub value: String,
    #[arg(long)]
    pub rationale: String,
    #[arg(long)]
    pub reviewer: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum ReviewCommand {
    /// Approve a knowledge item
    Approve(ReviewActionArgs),
    /// Reject a knowledge item
    Reject(ReviewActionArgs),
    /// Defer a knowledge item
    Defer(ReviewActionArgs),
    /// Comment on a knowledge item
    Comment(ReviewActionArgs),
    /// Edit a knowledge item field
    Edit(ReviewEditArgs),
}
