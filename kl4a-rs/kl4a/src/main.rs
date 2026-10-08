//! `kl4a`: one dispatcher in front of the active knowledge-bundle tools. Rust
//! equivalent of `tools/kl4a/kl4a/cli.py` (background only -- this is a fresh
//! implementation using idiomatic Rust / this workspace's own conventions, not a
//! line-for-line port of that argparse-based dispatcher).
//!
//!     kl4a --use sopkb <command> [args...]
//!     kl4a --use codekb <command> [args...]
//!
//! `--use <tool>` picks which tool runs; everything after it is forwarded to that
//! tool's own CLI entry point unchanged -- same subcommands, same flags, same
//! --help, same exit codes.
//!
//! Dispatch mechanism per tool (see the task report for verification evidence):
//!   - `codekb`: a true in-process call (`codekb::cli::main`).
//!   - `sopkb`, every subcommand except `serve` / `mcp serve`: also a true
//!     in-process call, via `sopkb_cli::cli::Cli::try_parse_from` +
//!     `sopkb_cli::execute`, reproducing `sopkb-cli.exe`'s own `main.rs` contract
//!     (see `run_sopkb_in_process`).
//!   - `sopkb serve` / `sopkb mcp serve`: the one deliberate exception. See
//!     `spawn_sibling`'s doc comment for why and how.

use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

const USAGE: &str = "\
kl4a - unified dispatcher for the kl4a knowledge-bundle tools

USAGE:
    kl4a --use <sopkb|codekb> <command> [args...]
    kl4a --help

Each tool's own subcommands and flags are documented via:
    kl4a --use <tool> --help
";

fn main() -> ExitCode {
    let argv: Vec<String> = env::args().skip(1).collect();
    ExitCode::from(run(&argv) as u8)
}

/// All the top-level argument handling kl4a does on its own behalf, before
/// forwarding to a tool. Returns a process exit code rather than `ExitCode`
/// directly so every branch (including the three tool dispatchers, which
/// already return a plain `i32`) shares one type.
fn run(argv: &[String]) -> i32 {
    match argv.first().map(String::as_str) {
        // Missing --use entirely: usage + error, exit 1.
        None => {
            print!("{USAGE}");
            eprintln!("\nerror: --use is required (sopkb or codekb)");
            1
        }
        // --help with no --use: kl4a's own help, exit 0.
        Some("--help") | Some("-h") => {
            print!("{USAGE}");
            0
        }
        Some("--use") => {
            let Some(tool) = argv.get(1) else {
                // --use with no tool name at all is the same class of usage
                // error as an unrecognized tool name below -- exit 2, clap's
                // own convention for a malformed invocation.
                eprintln!("error: --use requires a tool name (sopkb or codekb)");
                eprint!("{USAGE}");
                return 2;
            };
            // Note: a `--help`/`-h` appearing anywhere in `rest` is NOT
            // special-cased here -- it is forwarded untouched, exactly like
            // every other argument, to the chosen tool's own parser, which
            // already handles it (its own `--help` behavior, exit 0).
            let rest = &argv[2..];
            match tool.as_str() {
                "codekb" => codekb::cli::main(rest),
                "sopkb" => run_sopkb(rest),
                other => {
                    eprintln!("error: unknown tool {other:?} for --use (expected sopkb or codekb)");
                    eprint!("{USAGE}");
                    2
                }
            }
        }
        Some(other) => {
            eprintln!("error: unexpected argument {other:?} (expected --use <tool> or --help)");
            eprint!("{USAGE}");
            1
        }
    }
}

/// `--use sopkb <rest>`: dispatch into the sopkb tool. `serve` and `mcp serve`
/// have no equivalent in `sopkb_cli::cli::Command` -- that crate's own doc
/// comments say so explicitly -- so those two are spawned as a sibling
/// subprocess; everything else goes through the same in-process
/// parse-then-execute call `sopkb-cli.exe` itself makes.
fn run_sopkb(rest: &[String]) -> i32 {
    match rest.first().map(String::as_str) {
        Some("serve") => spawn_sibling("sopkb-server", &rest[1..]),
        Some("mcp") if rest.get(1).map(String::as_str) == Some("serve") => {
            // Strip the "mcp serve" prefix: `sopkb-mcp`'s own argv shape is
            // `sopkb-mcp <bundle_dir> [--enable-review-notes]` (see
            // `sopkb-rust/bin/sopkb-mcp/src/main.rs`), it does not itself take
            // a "serve" (or "mcp") token -- that phrasing only exists at the
            // `kl4a --use sopkb ...` dispatch level, mirroring the two-word
            // `mcp serve` subcommand shape the original Python sopkb CLI uses.
            spawn_sibling("sopkb-mcp", &rest[2..])
        }
        _ => run_sopkb_in_process(rest),
    }
}

/// Reproduces `kl4a-rs/sopkb-rust/bin/sopkb-cli/src/main.rs::main` byte for
/// byte: same success/error contract, same stdout text, same `error:
/// {message}` stderr format on failure, same clap usage-error behavior (exit
/// code 2) for a bad subcommand/flag. Read-only dependency on `sopkb_cli` --
/// nothing under `kl4a-rs/sopkb-rust/**` is modified to make this work.
fn run_sopkb_in_process(rest: &[String]) -> i32 {
    use clap::Parser;
    // `sopkb-cli`'s own `Cli::parse()` reads `std::env::args()`, whose first
    // element is the program name; `rest` doesn't include one (same
    // convention as `codekb::cli::main`'s `argv`), so a
    // placeholder is prepended before handing off to clap, which expects one.
    let argv = std::iter::once("sopkb-cli".to_string()).chain(rest.iter().cloned());
    let cli = match sopkb_cli::cli::Cli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(err) => {
            // Mirrors sopkb-cli's own main.rs: usage/help to the appropriate
            // stream, exit code 0 for --help/-h, 2 for a genuine usage error.
            err.print().ok();
            return err.exit_code();
        }
    };
    match sopkb_cli::execute(cli.command) {
        Ok(success) => {
            println!("{}", success.text);
            success.exit_code
        }
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}

/// Spawns an already-built sibling binary (`sopkb-server` or `sopkb-mcp`) as a
/// child process, inheriting stdio, and waits for/propagates its exit code.
///
/// This is the one deliberate exception to every other dispatch path in this
/// file, which calls straight into an in-process library function.
/// `sopkb-cli`'s own `Cli`/`execute()` -- in the separate `sopkb-rust`
/// workspace this crate must not modify -- has no `serve`/`mcp` command at
/// all: serving lives only in the standalone `sopkb-server` (axum/tokio async
/// HTTP server) and `sopkb-mcp` (stdio JSON-RPC loop) binaries. Linking this
/// synchronous argv-forwarding dispatcher against either directly would mean
/// pulling an async runtime (and, for `sopkb-server`, its whole web stack)
/// into every `kl4a` invocation just to serve two subcommands that are
/// long-running and stdio-owning by nature anyway -- a subprocess is the
/// natural boundary here, not a limitation being routed around.
///
/// Binary location: `cargo build --workspace` from `kl4a-rs/` places every
/// binary *built as part of that invocation* (codekb, kl4a) in
/// `kl4a-rs/target/<profile>/`. `sopkb-server`/`sopkb-mcp` are never a
/// dependency of this crate -- only the `sopkb-cli` *library* is, for the
/// in-process path above -- so they are never built by that invocation and
/// never land in `kl4a-rs/target/`. They only exist in
/// `kl4a-rs/sopkb-rust/target/<profile>/`, produced by building the separate
/// `sopkb-rust` workspace directly (confirmed empirically while building this
/// crate: `kl4a-rs/target/debug/` holds `codekb.exe` but not
/// `sopkb-*`; `kl4a-rs/sopkb-rust/target/debug/` holds all three `sopkb-*`
/// binaries but not `codekb`). So the sibling binary is located
/// relative to `sopkb-rust`'s own target directory, not this binary's own: walk
/// up from the running `kl4a` executable's path to the `kl4a-rs/` directory,
/// then descend into `sopkb-rust/target/<profile>/`, reusing the same profile
/// name (`debug`/`release`) this `kl4a` binary itself was built under (both
/// workspaces use cargo's standard profile directory names, so `kl4a` and
/// `sopkb-server`/`sopkb-mcp` built with the same `--release`-ness land under
/// matching profile directory names in their respective `target/`).
fn spawn_sibling(name: &str, args: &[String]) -> i32 {
    let path = match sibling_binary_path(name) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("error: {message}");
            return 1;
        }
    };
    match Command::new(&path).args(args).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            eprintln!("error: failed to run {}: {err}", path.display());
            1
        }
    }
}

fn sibling_binary_path(name: &str) -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| format!("could not determine current executable path: {e}"))?;
    // `exe` is `.../kl4a-rs/target/<profile>/kl4a[.exe]`.
    let profile_dir = exe
        .parent()
        .ok_or_else(|| format!("executable path {} has no parent directory", exe.display()))?;
    let profile_name = profile_dir
        .file_name()
        .ok_or_else(|| format!("could not read build profile directory name from {}", profile_dir.display()))?;
    let target_dir = profile_dir
        .parent()
        .ok_or_else(|| format!("could not find target/ directory above {}", profile_dir.display()))?;
    let kl4a_rs_dir = target_dir
        .parent()
        .ok_or_else(|| format!("could not find kl4a-rs/ directory above {}", target_dir.display()))?;
    let candidate = kl4a_rs_dir
        .join("sopkb-rust")
        .join("target")
        .join(profile_name)
        .join(format!("{name}{}", env::consts::EXE_SUFFIX));
    if candidate.is_file() {
        Ok(candidate)
    } else {
        Err(format!(
            "could not find {name} at {} -- build the sopkb-rust workspace first \
             (`cargo build --workspace` from kl4a-rs/sopkb-rust)",
            candidate.display()
        ))
    }
}
