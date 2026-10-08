//! HTTP transport seam. `UreqTransport` is the real client; tests and the
//! recorded-response harness (Phase 7/8 mining/agent gates) inject a `MockTransport`
//! instead so no network call is ever made in `cargo test`.

use std::cell::RefCell;
use std::io::Read;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Connection-level failure (DNS, refused, TLS, timeout). These propagate unwrapped
/// in the Python original -- only a non-2xx HTTP status gets the "Azure OpenAI ...
/// request failed" wrapper at the call site, so this type carries no formatting of
/// its own beyond the underlying transport's message.
#[derive(Debug, Clone)]
pub struct TransportIoError(pub String);

impl std::fmt::Display for TransportIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait Transport {
    fn post_json(&self, request: HttpRequest) -> Result<HttpResponse, TransportIoError>;
}

pub struct UreqTransport;

impl Transport for UreqTransport {
    fn post_json(&self, request: HttpRequest) -> Result<HttpResponse, TransportIoError> {
        let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(request.timeout_secs)).build();
        let mut req = agent.post(&request.url);
        for (k, v) in &request.headers {
            req = req.set(k, v);
        }
        match req.send_bytes(&request.body) {
            Ok(resp) => {
                let status = resp.status();
                let mut buf = Vec::new();
                resp.into_reader().read_to_end(&mut buf).map_err(|e| TransportIoError(e.to_string()))?;
                Ok(HttpResponse { status, body: buf })
            }
            // A non-2xx status is a normal (if unwelcome) HTTP response, not a
            // transport failure -- surface it as one so the caller applies the
            // "Azure OpenAI ... request failed: HTTP {status}: ..." wrapping itself.
            Err(ureq::Error::Status(status, resp)) => {
                let mut buf = Vec::new();
                let _ = resp.into_reader().read_to_end(&mut buf);
                Ok(HttpResponse { status, body: buf })
            }
            Err(ureq::Error::Transport(t)) => Err(TransportIoError(t.to_string())),
        }
    }
}

/// Canned-response double: records every outbound request and returns queued
/// results in order, so the request-building/response-parsing logic is testable
/// without a socket. This is also the seam Phase 7/8's recorded-response harness
/// should reuse to replay a once-captured real Azure response in both the Python
/// and Rust test runs.
///
/// `ok`/`err` queue exactly one response each (the original, single-call contract
/// every existing test relies on); `sequence` queues several, popped in call order
/// -- needed for a multi-turn caller (`sopkb_agent::react`'s tool-use loop) whose
/// test has to script "chunk request 1 -> a tool call, request 2 -> the final
/// answer" rather than one fixed response repeated.
#[derive(Default)]
pub struct MockTransport {
    results: RefCell<std::collections::VecDeque<Result<HttpResponse, TransportIoError>>>,
    requests: RefCell<Vec<HttpRequest>>,
}

impl MockTransport {
    pub fn ok(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self::sequence(vec![Ok(HttpResponse { status, body: body.into() })])
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self::sequence(vec![Err(TransportIoError(message.into()))])
    }

    pub fn sequence(results: Vec<Result<HttpResponse, TransportIoError>>) -> Self {
        Self { results: RefCell::new(results.into()), requests: RefCell::new(Vec::new()) }
    }

    pub fn last_request(&self) -> Option<HttpRequest> {
        self.requests.borrow().last().cloned()
    }

    /// Every request sent so far, in order -- lets a multi-turn test inspect the
    /// SECOND (or later) call's body (e.g. to confirm a tool result was actually
    /// folded into the next prompt), not just the most recent one.
    pub fn all_requests(&self) -> Vec<HttpRequest> {
        self.requests.borrow().clone()
    }
}

impl Transport for MockTransport {
    fn post_json(&self, request: HttpRequest) -> Result<HttpResponse, TransportIoError> {
        self.requests.borrow_mut().push(request.clone());
        self.results.borrow_mut().pop_front().expect("MockTransport called more times than responses were queued")
    }
}
