//! SSE replacement for the Tauri event bus (`desktop-tauri/src-tauri/src/events.rs`).
//! Same five topics/payload shapes that crate's `emit_*` helpers publish
//! (`bundle://state-changed`, `bundle://index-changed`, `ingest://progress`,
//! `agent://progress`, `settings://changed`) -- kept as a parallel implementation
//! rather than a shared dependency for the same reason `error.rs` is: desktop-tauri
//! is a standalone Cargo project this workspace does not (and must not) depend on.
//!
//! One process-wide `tokio::sync::broadcast` channel. A slow/absent subscriber never
//! blocks a publisher (`broadcast::Sender::send` is non-blocking; it only fails when
//! there are zero receivers, which every publish call here treats as a normal,
//! ignorable outcome -- matches the Tauri original's own "emission is always
//! best-effort" rule).

use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use futures_util::stream::Stream;
use serde::Serialize;
use serde_json::Value;
use std::convert::Infallible;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;

const CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone, Serialize)]
pub struct ServerEvent {
    pub topic: &'static str,
    pub payload: Value,
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<ServerEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { sender }
    }

    fn publish(&self, topic: &'static str, payload: Value) {
        let _ = self.sender.send(ServerEvent { topic, payload });
    }

    pub fn bundle_state_changed(&self, key: &str, scope: &[&str]) {
        self.publish("bundle://state-changed", serde_json::json!({ "key": key, "scope": scope }));
    }

    pub fn bundle_index_changed(&self, bundles_root: &str) {
        self.publish("bundle://index-changed", serde_json::json!({ "bundles_root": bundles_root }));
    }

    pub fn ingest_progress(&self, step: &str, status: &str, detail: impl Into<String>) {
        self.publish("ingest://progress", serde_json::json!({ "step": step, "status": status, "detail": detail.into() }));
    }

    pub fn agent_progress(&self, phase: &str, detail: impl Into<String>) {
        self.publish("agent://progress", serde_json::json!({ "phase": phase, "detail": detail.into() }));
    }

    pub fn settings_changed(&self) {
        self.publish("settings://changed", serde_json::json!({}));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.sender.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// `GET /api/events` -- one SSE stream carrying every topic; the client filters by
/// `event.topic` (JSON field, not the SSE `event:` line -- kept as one event type
/// named `sopkb` so a lagged/dropped-message gap, reported by `BroadcastStream` as
/// `Err(Lagged(n))`, can be skipped without terminating the whole connection).
pub fn sse_stream(bus: &EventBus) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(bus.subscribe()).filter_map(|item| match item {
        Ok(event) => serde_json::to_string(&event).ok().map(|json| Ok(Event::default().event("sopkb").data(json))),
        Err(_lagged) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub async fn events_handler(bus: axum::extract::State<crate::state::AppState>) -> impl IntoResponse {
    sse_stream(&bus.events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_with_no_subscribers_does_not_panic() {
        let bus = EventBus::new();
        bus.bundle_state_changed("b1", &["items"]);
        bus.settings_changed();
    }

    #[tokio::test]
    async fn a_subscriber_receives_a_published_event() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        bus.ingest_progress("mine", "done", "3 items");
        let received = rx.recv().await.unwrap();
        assert_eq!(received.topic, "ingest://progress");
        assert_eq!(received.payload["step"], serde_json::json!("mine"));
        assert_eq!(received.payload["detail"], serde_json::json!("3 items"));
    }
}
