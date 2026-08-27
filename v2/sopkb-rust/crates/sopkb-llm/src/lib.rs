//! Responses-API client with two distinct call paths (author_call, chat_call). See docs/port/PORT_PLAN.md §3.1 (sopkb-llm) and §6.8-6.9 (Phase 7-8).
//!
//! `author_call` (okf_author.py, mining) has no `reasoning` key and no
//! `status == "incomplete"` guard; `chat_call` (agent_chat.py) has both. Both flatten
//! a `[{role, content}]` message list (built by the calling crate) into
//! `instructions`/`input` strings and POST to `{base_url}/responses`. The `Transport`
//! trait is the network seam: `UreqTransport` for real calls, `MockTransport` for
//! tests and the recorded-response harness (no live LLM call belongs in `cargo test`).

mod author_call;
mod chat_call;
mod message;
mod pyrepr;
mod request;
mod response;
mod transport;

pub use author_call::{author_call, author_call_with_transport};
pub use chat_call::{chat_call, chat_call_with_transport};
pub use message::{flatten_messages, Message};
pub use request::{azure_responses_url, compact_body_no_reasoning, compact_body_with_reasoning, parse_py_int};
pub use transport::{HttpRequest, HttpResponse, MockTransport, Transport, TransportIoError, UreqTransport};
