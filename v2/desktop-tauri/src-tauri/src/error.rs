//! Wire-shape error mapping. PORT_PLAN.md §4.0:
//! `SopkbError { kind: "NotFound"|"InvalidInput"|"Conflict"|"MissingConfiguration"
//!             |"Upstream"|"Io"|"Format", message, detail? }`.
//!
//! `sopkb_core::error::SopkbError` has 5 variants (`Io`, `Value`, `NotFound`, `Parse`,
//! `Conflict`) that don't map 1:1 onto the 7-variant wire enum. This module is the one
//! place that reconciles them: `Io -> "Io"`, `NotFound -> "NotFound"`,
//! `Conflict -> "Conflict"`, `Parse -> "Format"` (a parse failure is a format problem,
//! not a distinct wire concept), `Value -> "InvalidInput"` (the overwhelming majority
//! of `SopkbError::Value` call sites are validation-shaped: "missing required field",
//! "field is not editable", "confidence must be between 0 and 1"). `Value` messages
//! that are actually about missing LLM configuration (`sopkb_config::require`'s
//! "Missing X. Configure a model profile...") are pattern-matched onto
//! `MissingConfiguration` since that's a materially different UI treatment (link to
//! Settings, not "you typed something wrong"). `Upstream` is reserved for LLM/HTTP
//! failures (also `SopkbError::Value` on the wire today -- `sopkb-llm` has no distinct
//! error variant either); detected the same way, by message shape.

use serde::Serialize;
use sopkb_core::error::SopkbError;

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub kind: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self { kind, message: message.into(), detail: None }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new("InvalidInput", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("NotFound", message)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}

/// Heuristics for `Value`'s sub-classification, since the underlying enum does not
/// distinguish "you typed something wrong" from "the LLM call failed" from "no model
/// profile is configured" -- all three currently surface as `SopkbError::Value(String)`
/// with only the message text to go on.
fn classify_value(message: &str) -> &'static str {
    if message.starts_with("Missing ") && message.contains("Configure a model profile") {
        "MissingConfiguration"
    } else if message.contains("request failed")
        || message.contains("HTTP")
        || message.contains("response")
        || message.contains("timed out")
        || message.contains("status ==")
        || message.contains("incomplete")
    {
        "Upstream"
    } else {
        "InvalidInput"
    }
}

impl From<SopkbError> for AppError {
    fn from(err: SopkbError) -> Self {
        let message = err.to_string();
        let kind = match &err {
            SopkbError::Io(_) => "Io",
            SopkbError::NotFound(_) => "NotFound",
            SopkbError::Conflict(_) => "Conflict",
            SopkbError::Parse(_) => "Format",
            SopkbError::Value(_) => classify_value(&message),
        };
        AppError { kind, message, detail: None }
    }
}

pub type CmdResult<T> = Result<T, AppError>;
