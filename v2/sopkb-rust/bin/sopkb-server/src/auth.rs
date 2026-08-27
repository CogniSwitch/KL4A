//! Bearer-token auth middleware. Every route except `/health` requires
//! `Authorization: Bearer <token>` matching the server's own generated token
//! (`token.rs`). Constant-time comparison (`subtle::ConstantTimeEq`) so response
//! timing can't be used to guess the token byte-by-byte.

use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;
use subtle::ConstantTimeEq;

use crate::error::ApiError;
use crate::state::AppState;

fn extract_bearer(request: &Request) -> Option<&str> {
    request.headers().get(AUTHORIZATION)?.to_str().ok()?.strip_prefix("Bearer ")
}

fn tokens_match(provided: &str, expected: &str) -> bool {
    // Constant-time equality requires equal-length inputs; a length mismatch alone
    // is not itself sensitive information here (token length is effectively public,
    // fixed at generation time -- see `token.rs`), so a cheap length check up front
    // is fine and avoids `ct_eq` panicking/misbehaving on mismatched lengths.
    provided.len() == expected.len() && bool::from(provided.as_bytes().ct_eq(expected.as_bytes()))
}

pub async fn require_bearer_token(State(state): State<AppState>, request: Request, next: Next) -> Result<Response, ApiError> {
    match extract_bearer(&request) {
        Some(provided) if tokens_match(provided, &state.token) => Ok(next.run(request).await),
        Some(_) => Err(ApiError::unauthorized("invalid bearer token")),
        None => Err(ApiError::unauthorized("missing Authorization: Bearer <token> header")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_match_requires_exact_equality() {
        assert!(tokens_match("abc123", "abc123"));
        assert!(!tokens_match("abc123", "abc124"));
        assert!(!tokens_match("abc12", "abc123"));
        assert!(!tokens_match("", "abc123"));
    }

    #[test]
    fn extract_bearer_reads_the_authorization_header() {
        let request = Request::builder().header(AUTHORIZATION, "Bearer secret-token").body(axum::body::Body::empty()).unwrap();
        assert_eq!(extract_bearer(&request), Some("secret-token"));
    }

    #[test]
    fn extract_bearer_none_without_the_bearer_prefix() {
        let request = Request::builder().header(AUTHORIZATION, "Basic dXNlcjpwYXNz").body(axum::body::Body::empty()).unwrap();
        assert_eq!(extract_bearer(&request), None);
    }

    #[test]
    fn extract_bearer_none_without_any_header() {
        let request = Request::builder().body(axum::body::Body::empty()).unwrap();
        assert_eq!(extract_bearer(&request), None);
    }
}
