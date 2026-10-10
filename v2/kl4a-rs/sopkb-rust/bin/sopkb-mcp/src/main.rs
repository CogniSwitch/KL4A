//! Standalone stdio JSON-RPC server (JSONL framing, 16 read-only + 1 gated tool). Shipped inside the app bundle,
//! discoverable via a Tauri command surfacing its invocation string (docs/port/DECISIONS.md Q4). See docs/port/PORT_PLAN.md §6.12 (Phase 10).
//!
//! CLI shape: `sopkb-mcp <bundle_dir> [--enable-review-notes]`.

mod jsonrpc;
mod tools;

use std::env;
use std::io::{self, BufReader};
use std::path::PathBuf;
use std::process::ExitCode;

struct CliArgs {
    bundle_dir: PathBuf,
    enable_review_notes: bool,
}

fn parse_args(args: &[String]) -> std::result::Result<CliArgs, String> {
    let mut bundle_dir: Option<PathBuf> = None;
    let mut enable_review_notes = false;
    for arg in args {
        if arg == "--enable-review-notes" {
            enable_review_notes = true;
        } else if bundle_dir.is_none() {
            bundle_dir = Some(PathBuf::from(arg));
        } else {
            return Err(format!("unexpected argument: {arg}"));
        }
    }
    match bundle_dir {
        Some(bundle_dir) => Ok(CliArgs { bundle_dir, enable_review_notes }),
        None => Err("usage: sopkb-mcp <bundle_dir> [--enable-review-notes]".to_string()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let cli = match parse_args(&args) {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };

    let stdin = BufReader::new(io::stdin());
    let stdout = io::stdout();
    match jsonrpc::serve_mcp_stdio(&cli.bundle_dir, cli.enable_review_notes, stdin, stdout.lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bundle_dir_only() {
        let cli = parse_args(&["C:/bundles/foo".to_string()]).unwrap();
        assert_eq!(cli.bundle_dir, PathBuf::from("C:/bundles/foo"));
        assert!(!cli.enable_review_notes);
    }

    #[test]
    fn parses_bundle_dir_and_flag_either_order() {
        let cli = parse_args(&["--enable-review-notes".to_string(), "b".to_string()]).unwrap();
        assert_eq!(cli.bundle_dir, PathBuf::from("b"));
        assert!(cli.enable_review_notes);

        let cli2 = parse_args(&["b".to_string(), "--enable-review-notes".to_string()]).unwrap();
        assert_eq!(cli2.bundle_dir, PathBuf::from("b"));
        assert!(cli2.enable_review_notes);
    }

    #[test]
    fn missing_bundle_dir_is_an_error() {
        assert!(parse_args(&[]).is_err());
        assert!(parse_args(&["--enable-review-notes".to_string()]).is_err());
    }

    #[test]
    fn extra_positional_argument_is_an_error() {
        assert!(parse_args(&["a".to_string(), "b".to_string()]).is_err());
    }
}
