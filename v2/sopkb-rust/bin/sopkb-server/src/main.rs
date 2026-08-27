use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use sopkb_server::{build_state, routes, static_files, token};

struct Args {
    bind: SocketAddr,
    regenerate_token: bool,
    static_dir: Option<PathBuf>,
    bundle_dir: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut bind: SocketAddr = "127.0.0.1:4173".parse().unwrap();
    let mut regenerate_token = false;
    let mut static_dir = None;
    let mut bundle_dir = std::env::var_os("SOPKB_BUNDLE_DIR").map(PathBuf::from);

    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--bind" => {
                if let Some(v) = raw.get(i + 1) {
                    if let Ok(parsed) = v.parse() {
                        bind = parsed;
                    } else {
                        eprintln!("warning: could not parse --bind value {v:?}, keeping default {bind}");
                    }
                }
                i += 1;
            }
            "--regenerate-token" => regenerate_token = true,
            "--static-dir" => {
                if let Some(v) = raw.get(i + 1) {
                    static_dir = Some(PathBuf::from(v));
                }
                i += 1;
            }
            "--bundle-dir" => {
                if let Some(v) = raw.get(i + 1) {
                    bundle_dir = Some(PathBuf::from(v));
                }
                i += 1;
            }
            other => eprintln!("warning: unrecognized argument {other:?}, ignoring"),
        }
        i += 1;
    }

    // `SOPKB_SERVER_BIND` env var as a fallback so a containerized/hosted deployment
    // can set the bind address without a wrapper script -- CLI flag still wins.
    if let Ok(env_bind) = std::env::var("SOPKB_SERVER_BIND") {
        if let Ok(parsed) = env_bind.parse() {
            bind = parsed;
        }
    }

    Args { bind, regenerate_token, static_dir, bundle_dir }
}

#[tokio::main]
async fn main() {
    let args = parse_args();

    let token = match token::load_or_create(args.regenerate_token) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("fatal: could not read/write the auth token at {}: {err}", token::path_for_display().display());
            std::process::exit(1);
        }
    };

    println!("sopkb-server v{}", env!("CARGO_PKG_VERSION"));
    println!("listening on http://{}", args.bind);
    println!("bearer token (also saved at {}):", token::path_for_display().display());
    println!("  {token}");
    println!("open http://{}/?token={token} to auto-fill it in the browser once", args.bind);

    let state = build_state(args.bundle_dir.as_deref(), token);
    let static_dir = args.static_dir.unwrap_or_else(static_files::default_dist_dir);

    let app: Router = routes::build_router(state).merge(static_files::serve(&static_dir));

    let listener = match tokio::net::TcpListener::bind(args.bind).await {
        Ok(l) => l,
        Err(err) => {
            eprintln!("fatal: could not bind {}: {err}", args.bind);
            std::process::exit(1);
        }
    };

    if let Err(err) = axum::serve(listener, app).await {
        eprintln!("fatal: server error: {err}");
        std::process::exit(1);
    }
}
