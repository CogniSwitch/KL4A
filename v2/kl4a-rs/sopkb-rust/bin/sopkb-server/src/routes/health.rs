use axum::Json;
use serde_json::{json, Value};

/// Unauthenticated by design (see `main.rs`'s router wiring) -- a health probe must
/// work before a caller has a token to present.
pub async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "service": "sopkb-server", "version": env!("CARGO_PKG_VERSION") }))
}
