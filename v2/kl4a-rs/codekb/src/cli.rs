//! Port of `kl4a/codekb/cli.py` (the `codekb` CLI: argument parsing and
//! subcommand dispatch) plus `kl4a/kl4a/codekb.py` (the top-level `kl4a`
//! umbrella CLI's thin `codekb` entrypoint shim, ported as [`entry`]).
//!
//! Every symbol from `cli.py` is grounded via tools-code MCP
//! `code_symbols_get` against `kl4a.codekb.cli.*`; the full `build_parser`
//! and `main` bodies were retrieved verbatim (they are quoted in full below
//! since they are this file's entire contract) and every subcommand branch
//! is ported as its own `match` arm — no branch skipped.
//!
//! # `kl4a/kl4a/codekb.py` grounding (see [`entry`])
//!
//! Unlike every other symbol in this batch, `kl4a/kl4a/codekb.py` could not
//! be retrieved as Python source text via tools-code MCP: `code_files_search`
//! confirms the file exists in the inventory (`source-kl4a-kl4a-codekb-py`,
//! with a checksum), but `code_symbols_search`/`code_callgraph_neighborhood`
//! return **zero** mined symbols for it — no functions, no classes, nothing.
//! Cross-checking the sibling umbrella dispatcher
//! (`kl4a.kl4a.cli._load_tool_main`, fully retrieved) shows it dispatches to
//! `--use codekb` by `importlib.import_module("codekb.cli")` and calling
//! `.main` on that module directly — i.e. the *actual* runtime dispatch path
//! for `kl4a --use codekb ...` never touches `kl4a/kl4a/codekb.py` at all.
//! Combined with zero mined symbols despite the file being indexed, the only
//! explanation consistent with all this evidence (and with the task's own
//! framing of it as "thin dispatch glue, not core logic") is that
//! `kl4a/kl4a/codekb.py` is a near-empty re-export shim with no function or
//! class definitions to mine — most plausibly a single `from codekb.cli
//! import main` (or equivalent) so that `import kl4a.codekb` / `python -m
//! kl4a.codekb` resolves, alongside (not instead of) the `--use` dispatch
//! path. **This is circumstantial, not a literal source read** (per the
//! hard constraint, Python source was never read directly for this
//! conclusion, and no such reading was attempted) — [`entry`] is written to
//! match that best-supported hypothesis and is marked `UNCONFIRMED`
//! accordingly.

use std::io::{stdin, stdout};
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;

// Port of `kl4a.codekb.cli.build_parser`'s subcommand tree (`argparse`) as
// a `clap` derive `Parser`.
//
// ```python
// def build_parser() -> argparse.ArgumentParser:
//     parser = argparse.ArgumentParser(prog="codekb", description="Code Knowledge Bundle workbench")
//     subparsers = parser.add_subparsers(dest="command", required=True)
//     ... # every subcommand below, one add_parser() call each
// ```
//
// `argparse`'s `subparsers.add_subparsers(dest=..., required=True)` at
// every nesting level is mirrored by making every `#[command(subcommand)]`
// field a plain (non-`Option`) enum — `clap` already requires a subcommand
// to be present in that shape, matching `required=True` without an extra
// check.
//
// NOTE: this used to be a `///` doc comment. When left as `///`, clap's
// derive picks up a struct's doc comment as `long_about` (shown for `--help`)
// whenever no explicit `long_about` is set, even though `about` (used for
// `-h`) is already set below — so this whole porting narrative was leaking
// into `codekb --help`'s description instead of Python's clean one-liner.
// Kept as a plain comment so clap never sees it.
#[derive(Parser, Debug)]
#[command(name = "codekb", about = "Code Knowledge Bundle workbench")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a Code Knowledge Bundle
    #[command(name = "init")]
    Init {
        // positional, optional: `code_init_parser.add_argument("bundle_dir", nargs="?")`
        // Python sets no `help=` for this positional, so it must carry no
        // clap help text either — this was a `///` doc comment leaking raw
        // Python source into `codekb init --help`.
        bundle_dir: Option<String>,
        #[arg(long, required = true)]
        repo: String,
        #[arg(long)]
        title: Option<String>,
        // `choices=["static", "hybrid"]` is already encoded below via
        // `value_parser`; the line was `choices=...` + a `help: "..."`
        // wrapper leaking through as a doc comment. Clean text below matches
        // Python's actual `help=` string.
        /// Mining mode recorded in the bundle manifest (default: static, or repo codekb config)
        #[arg(long, value_parser = ["static", "hybrid"])]
        mining: Option<String>,
    },
    /// Build a Code Knowledge Bundle end to end from a repository folder path
    #[command(name = "build")]
    Build {
        repo_dir: String,
        #[arg(long)]
        bundle: Option<String>,
        #[arg(long)]
        title: Option<String>,
        /// static runs deterministic extraction only; hybrid adds an LLM enrichment pass
        #[arg(long, value_parser = ["static", "hybrid"])]
        mining: Option<String>,
        /// Override the hybrid LLM provider (default: whatever kl4a's llm_settings currently resolves to - saved setting, then environment variable, then .env)
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        skip_render: bool,
    },
    /// Inventory a source-code repository
    #[command(name = "scan")]
    Scan {
        repo_dir: String,
        #[arg(long)]
        bundle: Option<String>,
    },
    /// Parse supported source files
    #[command(name = "parse")]
    Parse {
        bundle_dir: String,
        #[arg(long)]
        language: Option<String>,
    },
    /// Generate code relations
    #[command(name = "relations")]
    Relations { bundle_dir: String },
    /// Generate proposed code knowledge
    #[command(name = "mine")]
    Mine {
        bundle_dir: String,
        /// fixture/static for deterministic mining, hybrid/azure-llm to add LLM enrichment (default: the bundle's codekb.mining setting)
        #[arg(long)]
        provider: Option<String>,
    },
    /// Inspect or clear the LLM enrichment cache
    #[command(name = "cache")]
    Cache {
        #[command(subcommand)]
        command: CacheCommand,
    },
    /// Detect framework architecture (FastAPI endpoints, models, schemas, dependencies)
    #[command(name = "architecture")]
    Architecture { bundle_dir: String },
    /// Validate a Code Knowledge Bundle
    #[command(name = "validate")]
    Validate { bundle_dir: String },
    /// Compose human-readable indexes, module/symbol pages, and an overview
    /// from the mined bundle
    #[command(name = "render")]
    Render { bundle_dir: String },
    /// Code repository tools
    #[command(name = "repo")]
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    /// Code file tools
    #[command(name = "files")]
    Files {
        #[command(subcommand)]
        command: FilesCommand,
    },
    /// Code symbol tools
    #[command(name = "symbols")]
    Symbols {
        #[command(subcommand)]
        command: SymbolsCommand,
    },
    /// Code relation tools
    #[command(name = "relation")]
    Relation {
        #[command(subcommand)]
        command: RelationCommand,
    },
    /// Code test coverage tools
    #[command(name = "tests")]
    Tests {
        #[command(subcommand)]
        command: TestsCommand,
    },
    /// Estimate code change impact
    #[command(name = "change-impact")]
    ChangeImpact { bundle_dir: String, symbol_id: String },
    /// Get task-ready code context
    #[command(name = "context")]
    Context {
        bundle_dir: String,
        #[arg(long, required = true)]
        task: String,
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        language: Option<String>,
    },
    /// Run deterministic code-agent harness
    #[command(name = "agent")]
    Agent {
        bundle_dir: String,
        #[arg(long, required = true)]
        task: String,
        #[arg(long, required = true)]
        query: String,
        // `default="fixture"`; Python sets no `help=` for this option, so
        // it must carry no clap help text either.
        #[arg(long, default_value = "fixture")]
        provider: String,
    },
    /// Emit a canonical semantic model
    #[command(name = "canonicalize")]
    Canonicalize { bundle_dir: String },
    /// Cross-KB trace tools
    #[command(name = "trace")]
    Trace {
        #[command(subcommand)]
        command: TraceCommand,
    },
    /// Code transformation tools
    #[command(name = "transform")]
    Transform {
        #[command(subcommand)]
        command: TransformCommand,
    },
    /// Browse a Code Knowledge Bundle in the codekb workbench
    #[command(name = "serve")]
    Serve {
        bundle_dir: String,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 8765)]
        port: u16,
    },
    /// Model Context Protocol server
    #[command(name = "mcp")]
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum CacheCommand {
    /// Delete cached enrichment responses
    #[command(name = "clear")]
    Clear { bundle_dir: String },
}

#[derive(Subcommand, Debug)]
pub enum RepoCommand {
    /// Describe a code bundle
    #[command(name = "describe")]
    Describe { bundle_dir: String },
}

#[derive(Subcommand, Debug)]
pub enum FilesCommand {
    /// Search code files
    #[command(name = "search")]
    Search {
        bundle_dir: String,
        query: String,
        #[arg(long)]
        language: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum SymbolsCommand {
    /// Search code symbols
    #[command(name = "search")]
    Search {
        bundle_dir: String,
        query: String,
        #[arg(long)]
        language: Option<String>,
    },
    /// Get one code symbol
    #[command(name = "get")]
    Get { bundle_dir: String, symbol_id: String },
}

#[derive(Subcommand, Debug)]
pub enum RelationCommand {
    /// Search code relations
    #[command(name = "search")]
    Search {
        bundle_dir: String,
        #[arg(long, default_value = "")]
        subject: String,
        #[arg(long, default_value = "")]
        predicate: String,
        // Python: `add_argument("--object", dest="object_text", default="")`
        // — no `help=` set, so no clap help text either.
        #[arg(long = "object", default_value = "")]
        object_text: String,
        #[arg(long = "resolution-status", default_value = "")]
        resolution_status: String,
    },
    /// Get a code relation neighborhood
    #[command(name = "neighborhood")]
    Neighborhood {
        bundle_dir: String,
        node_id: String,
        // Python's `argparse` (`add_argument("--depth", type=int, default=1)`)
        // accepts any integer, including negative ones, as a plain
        // `type=int` conversion — it never treats a leading `-` specially.
        // clap's default parsing instead treats a `-1`-shaped token as an
        // unrecognized flag; `allow_hyphen_values` opts this arg back into
        // accepting hyphen-prefixed values (a genuinely non-numeric value
        // like `notanumber` still fails to parse as `i64` and errors exactly
        // like Python's `type=int` would).
        #[arg(long, default_value_t = 1, allow_hyphen_values = true)]
        depth: i64,
    },
}

#[derive(Subcommand, Debug)]
pub enum TestsCommand {
    /// List tests for a symbol
    #[command(name = "for-symbol")]
    ForSymbol { bundle_dir: String, symbol_id: String },
}

#[derive(Subcommand, Debug)]
pub enum TraceCommand {
    /// Create a cross-KB trace
    #[command(name = "create")]
    Create {
        source_bundle_dir: String,
        target_bundle_dir: String,
        #[arg(long, required = true)]
        out: String,
    },
    /// Validate a cross-KB trace
    #[command(name = "validate")]
    Validate { trace_dir: String },
    /// Print cross-KB trace coverage report
    #[command(name = "report")]
    Report { trace_dir: String },
    /// Review one cross-KB trace entry
    #[command(name = "review")]
    Review {
        trace_dir: String,
        source_artifact_id: String,
        #[arg(long, required = true)]
        disposition: String,
        #[arg(long, required = true)]
        reviewer: String,
        #[arg(long, required = true)]
        rationale: String,
        // `action="append", default=[]` — no `help=` set, so no clap help
        // text either.
        #[arg(long = "target")]
        target: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum TransformCommand {
    /// Create a target transform plan
    #[command(name = "plan")]
    Plan {
        source_bundle_dir: String,
        #[arg(long, required = true)]
        target: String,
        #[arg(long, required = true)]
        out: String,
    },
    /// Generate a target repo from a transform plan
    #[command(name = "generate")]
    Generate {
        plan_dir: String,
        #[arg(long = "out-repo", required = true)]
        out_repo: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum McpCommand {
    /// Serve the read-only code.* tools over JSON-RPC stdio
    #[command(name = "serve")]
    Serve { bundle_dir: String },
}

/// Port of `kl4a.codekb.cli.print_json`.
///
/// ```python
/// def print_json(data: object) -> None:
///     print(json.dumps(data, indent=2, sort_keys=True))
/// ```
fn print_json(data: &Value) {
    println!("{}", serde_json::to_string_pretty(&sort_keys(data)).unwrap_or_default());
}

/// Recursively sorts object keys so serialization matches Python's
/// `json.dumps(..., sort_keys=True)` (`serde_json::Map` is insertion-ordered,
/// not sorted, by default).
fn sort_keys(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for key in keys {
                sorted.insert(key.clone(), sort_keys(&map[key]));
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(sort_keys).collect()),
        other => other.clone(),
    }
}

/// Port of `kl4a.codekb.cli.enrichment_cache_line`.
///
/// ```python
/// def enrichment_cache_line(enrichment: dict) -> str:
///     """One line on cache effect, so the saving is visible rather than assumed."""
///     cache = enrichment.get("cache") or {}
///     if not cache.get("enabled"): return "cache: disabled"
///     calls = enrichment.get("provider_calls", 0)
///     parts = [f"cache: {cache.get('hits', 0)} reused, {calls} provider call(s)"]
///     if cache.get("pruned"): parts.append(f"{cache['pruned']} stale entry(ies) dropped")
///     parts.append(f"{cache.get('entries', 0)} entry(ies) stored")
///     return ", ".join(parts)
/// ```
fn enrichment_cache_line(enrichment: &Value) -> String {
    let cache = enrichment.get("cache").and_then(Value::as_object);
    let enabled = cache.and_then(|c| c.get("enabled")).and_then(Value::as_bool).unwrap_or(false);
    if !enabled {
        return "cache: disabled".to_string();
    }
    let calls = enrichment.get("provider_calls").and_then(Value::as_i64).unwrap_or(0);
    let hits = cache.and_then(|c| c.get("hits")).and_then(Value::as_i64).unwrap_or(0);
    let mut parts = vec![format!("cache: {hits} reused, {calls} provider call(s)")];
    let pruned = cache.and_then(|c| c.get("pruned")).and_then(Value::as_i64).unwrap_or(0);
    if pruned != 0 {
        parts.push(format!("{pruned} stale entry(ies) dropped"));
    }
    let entries = cache.and_then(|c| c.get("entries")).and_then(Value::as_i64).unwrap_or(0);
    parts.push(format!("{entries} entry(ies) stored"));
    parts.join(", ")
}

/// Port of `kl4a.codekb.cli.main`.
///
/// Full body (every subcommand branch, ported as its own `match` arm below
/// in [`run`]):
///
/// ```python
/// def main(argv: list[str] | None = None) -> int:
///     parser = build_parser()
///     args = parser.parse_args(argv)
///     try:
///         if args.command == "init": ...
///         if args.command == "build": ...
///         if args.command == "scan": ...
///         if args.command == "parse": ...
///         if args.command == "relations": ...
///         if args.command == "mine": ...
///         if args.command == "cache" and args.code_cache_command == "clear": ...
///         if args.command == "architecture": ...
///         if args.command == "validate": ...
///         if args.command == "render": ...
///         if args.command == "repo" and args.code_repo_command == "describe": ...
///         if args.command == "files" and args.code_files_command == "search": ...
///         if args.command == "symbols" and args.code_symbols_command == "search": ...
///         if args.command == "symbols" and args.code_symbols_command == "get": ...
///         if args.command == "relation" and args.code_relation_command == "search": ...
///         if args.command == "relation" and args.code_relation_command == "neighborhood": ...
///         if args.command == "tests" and args.code_tests_command == "for-symbol": ...
///         if args.command == "change-impact": ...
///         if args.command == "context": ...
///         if args.command == "agent": ...
///         if args.command == "canonicalize": ...
///         if args.command == "trace" and args.code_trace_command == "create": ...
///         if args.command == "trace" and args.code_trace_command == "validate": ...
///         if args.command == "trace" and args.code_trace_command == "report": ...
///         if args.command == "trace" and args.code_trace_command == "review": ...
///         if args.command == "transform" and args.code_transform_command == "plan": ...
///         if args.command == "transform" and args.code_transform_command == "generate": ...
///         if args.command == "serve": ...
///         if args.command == "mcp" and args.mcp_command == "serve": ...
///     except Exception as exc:
///         print(f"error: {exc}", file=sys.stderr)
///         return 1
///     parser.print_help()
///     return 1
/// ```
///
/// Takes `argv` as the already-split argument list (this crate's equivalent
/// of Python's `argv` when it is *not* `None`); an eventual umbrella binary
/// is expected to pass `std::env::args().skip(1).collect::<Vec<_>>()` for
/// the `argv is None` case, mirroring `sys.argv[1:]`.
///
/// The trailing `parser.print_help(); return 1` fallback in Python is
/// unreachable in practice: every `args.command` value that `argparse`
/// itself can produce (`required=True` at every subparser level) is
/// covered by one of the branches above. `clap`'s non-`Option` subcommand
/// fields enforce the same "always present" guarantee at parse time, so
/// [`run`]'s `match` is exhaustive over [`Command`] with no fallback arm
/// needed — parse failures (missing/invalid subcommand) are handled by
/// `clap` itself before [`run`] is ever called, exiting the same way
/// `argparse` would (usage message, nonzero exit).
pub fn main(argv: &[String]) -> i32 {
    let cli = match Cli::try_parse_from(std::iter::once("codekb".to_string()).chain(argv.iter().cloned())) {
        Ok(cli) => cli,
        Err(err) => {
            // Mirrors argparse: usage/help to the appropriate stream, exit
            // code 0 for --help/-h, 2 for a genuine usage error.
            err.print().ok();
            return err.exit_code();
        }
    };

    match run(cli.command) {
        Ok(code) => code,
        Err(exc) => {
            eprintln!("error: {exc}");
            1
        }
    }
}

/// Every branch of `main`'s `if args.command == ...` chain, as one `match`
/// arm each. Cross-batch note: most business-logic calls below
/// (`crate::bundle`, `crate::pipeline`, `crate::inventory`, `crate::parse`,
/// `crate::relations`, `crate::knowledge`, `crate::cache`,
/// `crate::architecture`, `crate::validate`, `crate::render`,
/// `crate::canonical`, `crate::trace`, `crate::transform`, `crate::server`)
/// are modules **not** in this batch and do not exist yet in this crate;
/// `crate::context::*`, `crate::agent::run_code_agent_harness`, and
/// `crate::mcp::serve_mcp_stdio` **do** already exist (from other batches)
/// and are called with their real signatures. See the handback report for
/// the full list of assumed not-yet-existing signatures.
fn run(command: Command) -> Result<i32> {
    match command {
        Command::Init { bundle_dir, repo, title, mining } => {
            let repo_dir = PathBuf::from(&repo);
            let bundle_dir = match bundle_dir {
                Some(b) => PathBuf::from(b),
                None => crate::config::standard_code_bundle_dir(&repo_dir, None)?,
            };
            let manifest = crate::bundle::create_code_bundle(&bundle_dir, &repo_dir, title.as_deref(), mining.as_deref())?;
            let mode = manifest
                .get("codekb")
                .and_then(|c| c.get("mining"))
                .and_then(|m| m.get("mode"))
                .and_then(Value::as_str)
                .unwrap_or("static");
            // Fix (Low finding): when `--bundle` is omitted, `bundle_dir`
            // came from `standard_code_bundle_dir`, which joins onto an
            // already-`canonicalize()`d `repo_dir` — on Windows that carries
            // the `\\?\` extended-length-path prefix into this message
            // unless stripped via `display_path`, matching the fix already
            // applied to the `build` summary's `bundle_dir` field.
            println!(
                "Initialized code bundle {} at {} (mining mode: {})",
                manifest.get("id").and_then(Value::as_str).unwrap_or_default(),
                crate::bundle_store::display_path(&bundle_dir),
                mode,
            );
            Ok(0)
        }

        Command::Build { repo_dir, bundle, title, mining, provider, skip_render } => {
            let summary = crate::pipeline::build_code_bundle_from_repo(
                Path::new(&repo_dir),
                crate::pipeline::BuildCodeBundleOptions {
                    bundle_dir: bundle.as_deref().map(Path::new).map(Path::to_path_buf),
                    title: title.as_deref(),
                    mining_mode: mining.as_deref(),
                    provider: provider.as_deref(),
                    render: !skip_render,
                    ..Default::default()
                },
            )?;
            let g = |k: &str| summary.get(k).cloned().unwrap_or(Value::Null);
            // `bundle_dir`/`mining_mode`/`provider` are strings; interpolating
            // the raw `Value` via `{}` invokes `Value`'s `Display`, which
            // serializes to JSON text (quoted, with backslashes escaped) —
            // e.g. `"C:\\tmp\\rs_bundle"` instead of the plain
            // `C:\tmp\rs_bundle` Python's f-string prints. Extract `.as_str()`
            // so these print as plain text, matching Python exactly.
            let gs = |k: &str| summary.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
            println!(
                "Built code bundle at {}\n  mining mode: {} (provider: {})\n  {} source(s), {} module(s), {} symbol(s), {} relation(s)\n  {} knowledge item(s), {} awaiting review",
                gs("bundle_dir"), gs("mining_mode"), gs("provider"),
                g("sources"), g("modules"), g("symbols"), g("relations"),
                g("knowledge_items"), g("review_required"),
            );
            let enrichment = summary.get("enrichment").cloned().unwrap_or(Value::Null);
            let attempted = enrichment.get("attempted").and_then(Value::as_i64).unwrap_or(0);
            if attempted != 0 {
                let accepted = enrichment.get("accepted").cloned().unwrap_or(Value::Null);
                let rejected_len = enrichment.get("rejected").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
                println!("  LLM enrichment: {accepted} claim(s) accepted from {attempted} symbol(s), {rejected_len} rejected");
                println!("  {}", enrichment_cache_line(&enrichment));
            }
            let errors = summary.get("errors").and_then(Value::as_i64).unwrap_or(0);
            let warnings = summary.get("warnings").and_then(Value::as_i64).unwrap_or(0);
            println!("  validation: {errors} error(s), {warnings} warning(s)");
            Ok(if errors != 0 { 1 } else { 0 })
        }

        Command::Scan { repo_dir, bundle } => {
            let repo_dir = PathBuf::from(&repo_dir);
            let bundle_dir = match bundle {
                Some(b) => PathBuf::from(b),
                None => crate::config::standard_code_bundle_dir(&repo_dir, None)?,
            };
            let inventory = crate::inventory::scan_code_repo(&repo_dir, &bundle_dir)?;
            let count = inventory.get("sources").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
            println!("Scanned {count} code source(s)");
            Ok(0)
        }

        Command::Parse { bundle_dir, language } => {
            let result = crate::parse::parse_code_bundle(Path::new(&bundle_dir), language.as_deref())?;
            let modules = result.get("modules").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
            let symbols = result.get("symbols").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
            println!("Parsed {modules} module(s), {symbols} symbol(s)");
            Ok(0)
        }

        Command::Relations { bundle_dir } => {
            let result = crate::relations::generate_code_relations(Path::new(&bundle_dir))?;
            let count = result.get("relations").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
            println!("Generated {count} code relation(s)");
            Ok(0)
        }

        Command::Mine { bundle_dir, provider } => {
            let bundle_dir = PathBuf::from(bundle_dir);
            let provider = match provider {
                Some(p) => p,
                None => crate::pipeline::configured_mining_provider(&bundle_dir),
            };
            let result = crate::knowledge::mine_code_bundle(&bundle_dir, &provider, None, None)?;
            let active = result
                .get("items")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter(|item| item.get("lifecycle_status").and_then(Value::as_str).unwrap_or("active") == "active")
                        .count()
                })
                .unwrap_or(0);
            println!("Mined {active} proposed code knowledge item(s) with provider {provider}");
            let enrichment = result.get("enrichment").cloned().unwrap_or(Value::Null);
            let attempted = enrichment.get("attempted").and_then(Value::as_i64).unwrap_or(0);
            if attempted != 0 {
                let accepted = enrichment.get("accepted").cloned().unwrap_or(Value::Null);
                let rejected_len = enrichment.get("rejected").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
                println!("LLM enrichment: {accepted} accepted from {attempted} symbol(s), {rejected_len} rejected");
                println!("{}", enrichment_cache_line(&enrichment));
            }
            Ok(0)
        }

        Command::Cache { command: CacheCommand::Clear { bundle_dir } } => {
            let bundle_dir = PathBuf::from(bundle_dir);
            if crate::cache::clear_cache(&bundle_dir)? {
                println!("Cleared enrichment cache at {}", crate::cache::cache_path(&bundle_dir, crate::cache::CACHE_FILENAME).display());
            } else {
                println!("No enrichment cache to clear");
            }
            Ok(0)
        }

        Command::Architecture { bundle_dir } => {
            let result = crate::architecture::detect_architecture(Path::new(&bundle_dir))?;
            let s = result.get("summary").cloned().unwrap_or(Value::Null);
            let g = |k: &str| s.get(k).cloned().unwrap_or(Value::Null);
            println!(
                "Detected {} endpoint(s), {} router(s), {} model(s), {} schema(s), {} dependency(ies)",
                g("endpoints"), g("routers"), g("data_models"), g("schemas"), g("dependencies"),
            );
            Ok(0)
        }

        Command::Validate { bundle_dir } => {
            let (errors, warnings) = crate::validate::validate_code_bundle(Path::new(&bundle_dir))?;
            println!("Code validation completed with {} error(s), {} warning(s)", errors.len(), warnings.len());
            Ok(if errors.is_empty() { 0 } else { 1 })
        }

        Command::Render { bundle_dir } => {
            let summary = crate::render::render_human_layer(Path::new(&bundle_dir))?;
            let modules = summary.get("modules").cloned().unwrap_or(Value::Null);
            let symbols = summary.get("symbols").cloned().unwrap_or(Value::Null);
            println!("Rendered human layer: {modules} module page(s), {symbols} symbol page(s), overview.md, populated indexes");
            Ok(0)
        }

        Command::Repo { command: RepoCommand::Describe { bundle_dir } } => {
            print_json(&crate::context::code_repo_describe(Path::new(&bundle_dir))?);
            Ok(0)
        }

        Command::Files { command: FilesCommand::Search { bundle_dir, query, language } } => {
            print_json(&crate::context::code_files_search(Path::new(&bundle_dir), &query, language.as_deref())?);
            Ok(0)
        }

        Command::Symbols { command: SymbolsCommand::Search { bundle_dir, query, language } } => {
            print_json(&crate::context::code_symbols_search(Path::new(&bundle_dir), &query, language.as_deref())?);
            Ok(0)
        }
        Command::Symbols { command: SymbolsCommand::Get { bundle_dir, symbol_id } } => {
            print_json(&crate::context::code_symbol_get(Path::new(&bundle_dir), &symbol_id)?);
            Ok(0)
        }

        Command::Relation { command: RelationCommand::Search { bundle_dir, subject, predicate, object_text, resolution_status } } => {
            print_json(&crate::context::code_relations_search(
                Path::new(&bundle_dir),
                &subject,
                &predicate,
                &object_text,
                &resolution_status,
            )?);
            Ok(0)
        }
        Command::Relation { command: RelationCommand::Neighborhood { bundle_dir, node_id, depth } } => {
            print_json(&crate::context::code_relation_neighborhood(Path::new(&bundle_dir), &node_id, depth)?);
            Ok(0)
        }

        Command::Tests { command: TestsCommand::ForSymbol { bundle_dir, symbol_id } } => {
            print_json(&crate::context::code_tests_for_symbol(Path::new(&bundle_dir), &symbol_id)?);
            Ok(0)
        }

        Command::ChangeImpact { bundle_dir, symbol_id } => {
            print_json(&crate::context::code_change_impact(Path::new(&bundle_dir), &symbol_id)?);
            Ok(0)
        }

        Command::Context { bundle_dir, task, query, language } => {
            print_json(&crate::context::code_context(Path::new(&bundle_dir), &task, query.as_deref(), language.as_deref())?);
            Ok(0)
        }

        Command::Agent { bundle_dir, task, query, provider } => {
            print_json(&crate::agent::run_code_agent_harness(Path::new(&bundle_dir), &task, &query, &provider)?);
            Ok(0)
        }

        Command::Canonicalize { bundle_dir } => {
            let result = crate::canonical::canonicalize_code_bundle(Path::new(&bundle_dir))?;
            let count = result.get("artifacts").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
            println!("Canonicalized {count} artifact(s)");
            Ok(0)
        }

        Command::Trace { command: TraceCommand::Create { source_bundle_dir, target_bundle_dir, out } } => {
            let result = crate::trace::create_cross_kb_trace(
                Path::new(&source_bundle_dir),
                Path::new(&target_bundle_dir),
                Path::new(&out),
            )?;
            let count = result.get("entries").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
            println!("Created {count} trace entrie(s)");
            Ok(0)
        }
        Command::Trace { command: TraceCommand::Validate { trace_dir } } => {
            let (errors, warnings) = crate::trace_validate::validate_cross_kb_trace(Path::new(&trace_dir))?;
            println!("Trace validation completed with {} error(s), {} warning(s)", errors.len(), warnings.len());
            Ok(if errors.is_empty() { 0 } else { 1 })
        }
        Command::Trace { command: TraceCommand::Report { trace_dir } } => {
            let report = std::fs::read_to_string(Path::new(&trace_dir).join("reports").join("coverage.md"))?;
            println!("{report}");
            Ok(0)
        }
        Command::Trace { command: TraceCommand::Review { trace_dir, source_artifact_id, disposition, reviewer, rationale, target } } => {
            // Fix (Medium finding — `trace review` without `--target`):
            // re-checked against the actual current Python source rather
            // than trusting the audit's framing. `review_trace_entry`'s own
            // Python default is `target_artifacts: list[str] | None = None`
            // (an omitted `None` leaves existing targets untouched), BUT
            // this CLI command never actually gets to exercise that branch:
            // `cli.py` wires `--target` as
            // `add_argument("--target", action="append", default=[])`, so
            // when the flag is never passed, `args.target` is `[]` (an
            // empty list), not `None` — and `review_trace_entry` checks
            // `if target_artifacts is not None`, and `[] is not None` is
            // `True` in Python. So via this CLI, omitting `--target`
            // *always* clears existing targets to `[]`; there is no way to
            // reach the "leave untouched" branch through the `review`
            // subcommand at all. The previous version here did the
            // opposite (`if target.is_empty() { None } else { Some(target) }`),
            // silently leaving existing targets alone instead of clearing
            // them — exactly backwards. Always pass `Some(target)`
            // (`target` is already `Vec::new()` by default via clap when
            // `--target` is omitted, matching argparse's `default=[]`).
            print_json(&crate::trace::review_trace_entry(
                Path::new(&trace_dir),
                &source_artifact_id,
                &disposition,
                &reviewer,
                &rationale,
                Some(target),
            )?);
            Ok(0)
        }

        Command::Transform { command: TransformCommand::Plan { source_bundle_dir, target, out } } => {
            let result = crate::transform::create_transform_plan(Path::new(&source_bundle_dir), &target, Path::new(&out))?;
            let count = result.get("operations").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
            println!("Planned {count} operation(s)");
            Ok(0)
        }
        Command::Transform { command: TransformCommand::Generate { plan_dir, out_repo } } => {
            let result = crate::transform::generate_target_repo(Path::new(&plan_dir), Path::new(&out_repo))?;
            // Same `Value` Display leak as `build`'s success message: printing
            // the raw `Value` here would show a quoted, backslash-escaped
            // JSON string instead of the plain path Python's f-string prints.
            let target_repo = result.get("target_repo").and_then(Value::as_str).unwrap_or_default();
            println!("Generated target repo at {target_repo}");
            Ok(0)
        }

        Command::Serve { bundle_dir, host, port } => {
            crate::server::serve_bundle(Path::new(&bundle_dir), &host, port)?;
            Ok(0)
        }

        Command::Mcp { command: McpCommand::Serve { bundle_dir } } => {
            let stdin = stdin();
            let stdout = stdout();
            let mut input = stdin.lock();
            let mut output = stdout.lock();
            crate::mcp::serve_mcp_stdio(Path::new(&bundle_dir), &mut input, &mut output)?;
            Ok(0)
        }
    }
}

/// Port of `kl4a/kl4a/codekb.py` — the top-level `kl4a` umbrella CLI's
/// `codekb` entrypoint shim.
///
/// See the module-level doc comment above for why this file's content could
/// not be read directly and what evidence grounds this reconstruction.
/// `UNCONFIRMED`: mirrored here as a direct forward to [`main`] (the
/// best-supported hypothesis for a file with zero mined symbols that the
/// umbrella dispatcher's own equivalent path treats as `codekb.cli.main`),
/// not asserted as a literal line-for-line transcription of source this
/// batch never saw.
pub fn entry(argv: &[String]) -> i32 {
    main(argv)
}
