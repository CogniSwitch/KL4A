//! Serves the built web frontend (`v2/frontend/dist-web/`, produced by `npm run
//! build:web` -- see that project's `package.json` -- deliberately NOT
//! `desktop-tauri/dist`, which is Tauri-specific and committed for a different
//! purpose). SPA fallback: any path that isn't a real file under the dist dir (i.e.
//! a client-side route like `/sources`) serves `index.html` instead of 404ing, so
//! the React router can take over.

use std::path::PathBuf;

use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

pub fn serve(dist_dir: &std::path::Path) -> Router {
    let index = dist_dir.join("index.html");
    // Deliberately `.fallback(...)`, NOT `.not_found_service(...)`: the latter forces
    // the response status to 404 even though the body IS a real, valid page (its
    // whole point is "serve this as a custom 404 page") -- confirmed by curl during
    // manual smoke testing (`GET /sources` returned the correct `index.html` body but
    // status 404). An SPA fallback route is not an error; the client-side router
    // renders the right screen from a normal 200, and a 404 status would be actively
    // wrong for e.g. a crawler or any client that checks status before parsing the body.
    let serve_dir = ServeDir::new(dist_dir).fallback(ServeFile::new(index));
    Router::new().fallback_service(serve_dir)
}

/// Best-effort default: `<repo>/v2/frontend/dist-web`, resolved relative to the
/// running executable's location during development (`cargo run`) -- a packaged
/// deployment should pass `--static-dir` explicitly instead of relying on this.
pub fn default_dist_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../frontend/dist-web")
}
