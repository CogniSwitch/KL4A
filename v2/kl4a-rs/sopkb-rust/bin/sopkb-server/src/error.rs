//! HTTP-shape error mapping, mirroring `desktop-tauri/src-tauri/src/error.rs`'s
//! `AppError { kind, message, detail }` wire shape so a client written against one
//! backend recognizes the other's errors -- kept as its own small copy rather than a
//! shared dependency since desktop-tauri is a standalone Cargo project outside this
//! workspace and pulls in Tauri types this crate must not depend on.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use sopkb_core::error::SopkbError;

#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub kind: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self { kind, message: message.into() }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new("InvalidInput", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("NotFound", message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new("Unauthorized", message)
    }

    fn status(&self) -> StatusCode {
        match self.kind {
            "NotFound" => StatusCode::NOT_FOUND,
            "InvalidInput" | "Conflict" | "Format" => StatusCode::BAD_REQUEST,
            "Unauthorized" => StatusCode::UNAUTHORIZED,
            "MissingConfiguration" => StatusCode::PRECONDITION_FAILED,
            "Upstream" => StatusCode::BAD_GATEWAY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        (status, Json(self)).into_response()
    }
}

/// Same classification heuristic as desktop-tauri's `error.rs::classify_value` --
/// `sopkb_core::error::SopkbError::Value` bundles together validation errors, missing-
/// LLM-config errors, and upstream HTTP failures under one string-only variant, and
/// heuristics on the message text are the only way to tell them apart. Kept in sync
/// manually since the two crates share no common error-mapping dependency.
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

impl From<SopkbError> for ApiError {
    fn from(err: SopkbError) -> Self {
        let message = err.to_string();
        let kind = match &err {
            SopkbError::Io(_) => "Io",
            SopkbError::NotFound(_) => "NotFound",
            SopkbError::Conflict(_) => "Conflict",
            SopkbError::Parse(_) => "Format",
            SopkbError::Value(_) => classify_value(&message),
        };
        ApiError { kind, message }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_maps_to_404() {
        assert_eq!(ApiError::not_found("x").status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn missing_configuration_is_classified_from_message_shape() {
        let err = ApiError::from(SopkbError::Value("Missing api_key. Configure a model profile in Settings.".to_string()));
        assert_eq!(err.kind, "MissingConfiguration");
    }

    #[test]
    fn upstream_is_classified_from_message_shape() {
        let err = ApiError::from(SopkbError::Value("request failed: status == 500".to_string()));
        assert_eq!(err.kind, "Upstream");
    }

    #[test]
    fn plain_value_error_is_invalid_input() {
        let err = ApiError::from(SopkbError::Value("confidence must be between 0 and 1".to_string()));
        assert_eq!(err.kind, "InvalidInput");
    }
}
