//! Real HTTP integration tests against a live bound TCP port -- not `oneshot`, per
//! the explicit requirement that at least the spine be proven over an actual socket,
//! the same transport a real browser/client uses.

use std::time::Duration;

use serde_json::{json, Value};
use sopkb_server::{build_state, routes};

struct TestServer {
    base_url: String,
    token: String,
    _bundle_dir_holder: tempfile::TempDir,
}

async fn spawn_server() -> TestServer {
    let root = tempfile::tempdir().unwrap();
    let bundle_dir = root.path().join("knowledge-bundles").join("bundle");
    sopkb_core::store::create_bundle(&bundle_dir, Some("Test Bundle")).unwrap();

    let token = "test-token-0123456789abcdef".to_string();
    let state = build_state(Some(root.path()), token.clone());
    state.workbench.select_bundle("bundle").unwrap();

    let app = routes::build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    // Give the spawned server task a moment to actually start accepting.
    tokio::time::sleep(Duration::from_millis(50)).await;

    TestServer { base_url: format!("http://{addr}"), token, _bundle_dir_holder: root }
}

#[tokio::test]
async fn health_works_without_any_auth_header() {
    let server = spawn_server().await;
    let resp = reqwest::get(format!("{}/health", server.base_url)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["ok"], json!(true));
}

#[tokio::test]
async fn authenticated_route_rejects_a_missing_token() {
    let server = spawn_server().await;
    let client = reqwest::Client::new();
    let resp = client.get(format!("{}/api/context", server.base_url)).send().await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn authenticated_route_rejects_a_wrong_token() {
    let server = spawn_server().await;
    let client = reqwest::Client::new();
    let resp = client.get(format!("{}/api/context", server.base_url)).bearer_auth("not-the-real-token").send().await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn authenticated_route_accepts_the_real_token() {
    let server = spawn_server().await;
    let client = reqwest::Client::new();
    let resp = client.get(format!("{}/api/context", server.base_url)).bearer_auth(&server.token).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn full_read_path_list_bundles_describe_sources_sections_knowledge() {
    let server = spawn_server().await;
    let client = reqwest::Client::new();

    let bundles: Value = client.get(format!("{}/api/bundles", server.base_url)).bearer_auth(&server.token).send().await.unwrap().json().await.unwrap();
    assert_eq!(bundles.as_array().unwrap().len(), 1);
    assert_eq!(bundles[0]["key"], json!("bundle"));

    let summary: Value =
        client.get(format!("{}/api/bundles/describe?key=bundle", server.base_url)).bearer_auth(&server.token).send().await.unwrap().json().await.unwrap();
    assert_eq!(summary["title"], json!("Test Bundle"));

    let sources: Value =
        client.get(format!("{}/api/sources?key=bundle", server.base_url)).bearer_auth(&server.token).send().await.unwrap().json().await.unwrap();
    assert!(sources.as_array().unwrap().is_empty());

    let sections: Value =
        client.get(format!("{}/api/sections?key=bundle", server.base_url)).bearer_auth(&server.token).send().await.unwrap().json().await.unwrap();
    assert!(sections.as_array().unwrap().is_empty());

    let items: Value =
        client.get(format!("{}/api/knowledge?key=bundle", server.base_url)).bearer_auth(&server.token).send().await.unwrap().json().await.unwrap();
    assert!(items.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn ingest_pipeline_over_a_real_source_dir_broadcasts_progress_over_sse() {
    let server = spawn_server().await;
    let client = reqwest::Client::new();

    let source_dir = tempfile::tempdir().unwrap();
    std::fs::write(source_dir.path().join("policy.md"), "# Policy\n\nContent here.\n").unwrap();

    // Subscribe to SSE BEFORE triggering the run so the "scan started" event isn't missed.
    let sse_resp = client.get(format!("{}/api/events", server.base_url)).bearer_auth(&server.token).send().await.unwrap();
    assert_eq!(sse_resp.status(), 200);
    let mut sse_body = sse_resp.bytes_stream();

    let run_body = json!({
        "source": {"kind": "folder", "path": source_dir.path().display().to_string()},
        "scan": true, "normalize": true, "mine": false, "validate": true, "export": false,
        "mine_provider": "fixture", "profile_id": null, "key": "bundle",
    });
    let run_task = {
        let base_url = server.base_url.clone();
        let token = server.token.clone();
        tokio::spawn(async move {
            reqwest::Client::new().post(format!("{base_url}/api/ingest/run")).bearer_auth(token).json(&run_body).send().await.unwrap()
        })
    };

    // Read SSE chunks until we've seen at least a "scan" and a "validate" progress event,
    // or time out -- proves progress is genuinely observable live, not just in the final result.
    use futures_util::StreamExt as _;
    let mut seen_scan = false;
    let mut seen_validate = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline && !(seen_scan && seen_validate) {
        let chunk = tokio::time::timeout(Duration::from_secs(2), sse_body.next()).await;
        let Ok(Some(Ok(bytes))) = chunk else { continue };
        let text = String::from_utf8_lossy(&bytes);
        if text.contains("ingest://progress") && text.contains("\"scan\"") {
            seen_scan = true;
        }
        if text.contains("ingest://progress") && text.contains("\"validate\"") {
            seen_validate = true;
        }
    }
    assert!(seen_scan, "expected to observe a scan progress event over SSE");
    assert!(seen_validate, "expected to observe a validate progress event over SSE");

    let resp = run_task.await.unwrap();
    assert_eq!(resp.status(), 200);
    let result: Value = resp.json().await.unwrap();
    assert_eq!(result["sources"], json!(1));
}
