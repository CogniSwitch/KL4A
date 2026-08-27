//! Ported CLI; exit code 1 on validation errors. Full subcommand surface: see
//! `docs/port/port-mapping-e-web-cli-contract.md` §5 and `docs/port/PORT_PLAN.md` §6.12.
//!
//! This binary is also the differential test harness's driver (`v2/sopkb-rust/fixtures/
//! harness/harness.py`, Phase 2 onward) -- it is invoked directly with `argv`, one
//! subcommand-shaped invocation per `script` line, and its stdout/stderr/exit code are
//! captured and diffed against the reference Python CLI's recorded output. That harness
//! drives the compiled binary, not this crate's library API, so nothing here needs to
//! special-case it -- normal argv parsing and the standard success/error contract below
//! are exactly what it expects.
//!
//! Contract: on success, print the command's result (JSON, or raw markdown for the
//! handful of report-reading commands) to stdout and exit 0 -- except `validate`, which
//! exits 1 when validation found any error. On failure, print `error: {message}` to
//! stderr and exit 1. A clap usage error (missing/invalid argument) prints clap's own
//! usage text and exits with clap's own exit code (2), which already satisfies "a clear
//! error, not a panic" for malformed invocations.
//!
//! `sopkb serve` (docs/port/DECISIONS.md Q1) and an `sopkb-cli mcp serve` convenience
//! alias over the standalone `sopkb-mcp` binary are both out of scope for this pass --
//! see the worktree's task notes / final report for why.

use clap::Parser;
use sopkb_cli::cli::Cli;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match sopkb_cli::execute(cli.command) {
        Ok(success) => {
            println!("{}", success.text);
            exit_code(success.exit_code)
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn exit_code(code: i32) -> ExitCode {
    ExitCode::from(code.clamp(0, 255) as u8)
}
