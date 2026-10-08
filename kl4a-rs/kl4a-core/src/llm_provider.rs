//! Port of `kl4a/kl4a/llm_provider.py`. Grounded via `tools-code` MCP
//! `code_symbols_get` against `source-kl4a-kl4a-llm-provider-py`.
//!
//! Network I/O: Python's `http_post_json` uses `urllib.request`, which
//! handles HTTPS via the system's OpenSSL/TLS stack for free. This port
//! mirrors that behavior with two transports in [`http_post_json`]: plain
//! `http://` is still hand-rolled HTTP/1.1 request/response framing over
//! `std::net::TcpStream` (unchanged from the original integration pass —
//! e.g. for a local Ollama/vLLM `OpenAICompatibleProvider` setup), while
//! `https://` is handled by the `ureq` crate (rustls-backed, so no system
//! OpenSSL dependency on Windows or elsewhere). All three real providers'
//! default base URLs are `https://`, and that path now makes real,
//! successful calls against them — this is no longer a scope boundary.

use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{Map, Value};

/// One chat message: `{"role": ..., "content": ...}`. Port of Python's
/// `Message = dict[str, str]` type alias used throughout `llm_provider.py`.
/// Modeled as `serde_json::Value` rather than `HashMap<String, String>` to
/// match the call sites already written elsewhere in this workspace
/// (`codekb::author::build_code_author_messages`,
/// `codekb::procedures::build_procedure_author_messages`), which build
/// `Vec<Value>` message lists.
pub type Message = Value;

/// Port of `kl4a.kl4a.llm_provider.ProviderError` (`kl4a/kl4a/llm_provider.py:39-40`).
///
/// ```python
/// class ProviderError(RuntimeError):
///     """A provider call failed in a way the user can usually act on."""
/// ```
#[derive(Debug, Clone)]
pub struct ProviderError(pub String);

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ProviderError {}

impl ProviderError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

/// Port of `DEFAULT_MAX_OUTPUT_TOKENS = 4096` (`kl4a/kl4a/llm_provider.py:23`).
pub const DEFAULT_MAX_OUTPUT_TOKENS: i64 = 4096;
/// Port of `DEFAULT_TIMEOUT_SECONDS = 60` (`kl4a/kl4a/llm_provider.py:24`).
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 60;

/// Port of `kl4a.kl4a.llm_provider.split_messages` (`kl4a/kl4a/llm_provider.py:43-46`).
///
/// ```python
/// def split_messages(messages: list[Message]) -> tuple[str, str]:
///     system = "\n\n".join(m["content"] for m in messages if m.get("role") == "system")
///     user = "\n\n".join(m["content"] for m in messages if m.get("role") == "user")
///     return system, user
/// ```
pub fn split_messages(messages: &[Message]) -> (String, String) {
    let content_of = |m: &Value| -> String {
        m.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string()
    };
    let role_is = |m: &Value, role: &str| -> bool {
        m.get("role").and_then(|v| v.as_str()) == Some(role)
    };
    let system = messages
        .iter()
        .filter(|m| role_is(m, "system"))
        .map(content_of)
        .collect::<Vec<_>>()
        .join("\n\n");
    let user = messages
        .iter()
        .filter(|m| role_is(m, "user"))
        .map(content_of)
        .collect::<Vec<_>>()
        .join("\n\n");
    (system, user)
}

/// Port of `kl4a.kl4a.llm_provider.http_post_json` (`kl4a/kl4a/llm_provider.py:440-461`).
/// Dispatches to [`http_post_json_https`] for `https://` URLs (via `ureq`)
/// and keeps the original raw-socket implementation below for `http://`.
pub fn http_post_json(
    url: &str,
    body: &Value,
    headers: &HashMap<String, String>,
    timeout_secs: u64,
    label: &str,
) -> Result<Value, ProviderError> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| ProviderError::new(format!("{label} request failed: invalid URL: {url}")))?;
    if scheme.eq_ignore_ascii_case("https") {
        return http_post_json_https(url, body, headers, timeout_secs, label);
    } else if !scheme.eq_ignore_ascii_case("http") {
        return Err(ProviderError::new(format!(
            "{label} request failed: unsupported URL scheme: {scheme}"
        )));
    }
    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(80)),
        None => (authority, 80),
    };

    let payload = serde_json::to_vec(body)
        .map_err(|exc| ProviderError::new(format!("{label} request failed: {exc}")))?;

    let mut request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\n",
        payload.len()
    );
    let mut has_content_type = false;
    for (key, value) in headers {
        if key.eq_ignore_ascii_case("content-type") {
            has_content_type = true;
        }
        request.push_str(&format!("{key}: {value}\r\n"));
    }
    if !has_content_type {
        request.push_str("Content-Type: application/json\r\n");
    }
    request.push_str("\r\n");

    let addr = format!("{host}:{port}");
    let stream = TcpStream::connect(&addr)
        .map_err(|exc| ProviderError::new(format!("{label} request failed: {exc}")))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(timeout_secs)))
        .ok();
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(timeout_secs)))
        .ok();
    let mut stream = stream;
    stream
        .write_all(request.as_bytes())
        .map_err(|exc| ProviderError::new(format!("{label} request failed: {exc}")))?;
    stream
        .write_all(&payload)
        .map_err(|exc| ProviderError::new(format!("{label} request failed: {exc}")))?;

    let mut response_bytes = Vec::new();
    stream
        .read_to_end(&mut response_bytes)
        .map_err(|exc| ProviderError::new(format!("{label} request failed: {exc}")))?;
    let response_text = String::from_utf8_lossy(&response_bytes);
    let header_end = response_text
        .find("\r\n\r\n")
        .ok_or_else(|| ProviderError::new(format!("{label} request failed: malformed HTTP response")))?;
    let status_line = response_text.lines().next().unwrap_or("");
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let response_body = &response_text[header_end + 4..];
    if !(200..300).contains(&status_code) {
        return Err(ProviderError::new(format!(
            "{label} request failed: HTTP {status_code}: {response_body}"
        )));
    }
    serde_json::from_str(response_body)
        .map_err(|exc| ProviderError::new(format!("{label} returned a non-JSON response: {exc}")))
}

/// `https://` transport for [`http_post_json`], via the `ureq` crate
/// (rustls-backed TLS, no system OpenSSL dependency). Mirrors Python's
/// `urllib.request`-based behavior: a single POST attempt (no retries), one
/// overall timeout covering connect+send+receive (Python's `urlopen(...,
/// timeout=...)` is likewise a single blocking-socket timeout, not phased),
/// headers passed through as given plus a default `Content-Type` if the
/// caller didn't set one, and on a non-2xx status the response body text is
/// included in the returned [`ProviderError`] (matching Python's
/// `HTTPError` handling, which reads and includes the error body).
fn http_post_json_https(
    url: &str,
    body: &Value,
    headers: &HashMap<String, String>,
    timeout_secs: u64,
    label: &str,
) -> Result<Value, ProviderError> {
    let payload = serde_json::to_vec(body)
        .map_err(|exc| ProviderError::new(format!("{label} request failed: {exc}")))?;

    let mut request = ureq::post(url);
    let mut has_content_type = false;
    for (key, value) in headers {
        if key.eq_ignore_ascii_case("content-type") {
            has_content_type = true;
        }
        request = request.header(key.as_str(), value.as_str());
    }
    if !has_content_type {
        request = request.header("Content-Type", "application/json");
    }

    let mut response = request
        .config()
        // Match the plain-http:// path below: surface the status code and
        // response body ourselves rather than having ureq turn a 4xx/5xx
        // into an opaque `Error::StatusCode` with no body attached.
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(timeout_secs)))
        .build()
        .send(payload)
        .map_err(|exc| ProviderError::new(format!("{label} request failed: {exc}")))?;

    let status_code = response.status().as_u16();
    let response_body = response
        .body_mut()
        .read_to_string()
        .map_err(|exc| ProviderError::new(format!("{label} request failed: {exc}")))?;

    if !(200..300).contains(&status_code) {
        return Err(ProviderError::new(format!(
            "{label} request failed: HTTP {status_code}: {response_body}"
        )));
    }
    serde_json::from_str(&response_body)
        .map_err(|exc| ProviderError::new(format!("{label} returned a non-JSON response: {exc}")))
}

/// Port of `kl4a.kl4a.llm_provider.Provider` (`kl4a/kl4a/llm_provider.py:49-126`).
/// Fields (`id`, `label`, `required_fields`, `env_names`, etc.) that are class
/// attributes in Python become [`ProviderMeta`] here, held per concrete
/// provider; behavior methods stay on the trait.
pub trait Provider {
    fn meta(&self) -> &'static ProviderMeta;
    fn settings(&self) -> &HashMap<String, String>;

    /// Port of `Provider.value` (`kl4a/kl4a/llm_provider.py:80-81`).
    fn value(&self, field: &str, default: &str) -> String {
        self.settings()
            .get(field)
            .filter(|v| !v.is_empty())
            .cloned()
            .unwrap_or_else(|| default.to_string())
            .trim()
            .to_string()
    }

    /// Port of `Provider.require` (`kl4a/kl4a/llm_provider.py:83-87`).
    fn require(&self, field: &str) -> Result<String, ProviderError> {
        let value = self.value(field, "");
        if value.is_empty() {
            return Err(ProviderError::new(format!("missing required setting: {field}")));
        }
        Ok(value)
    }

    /// Port of `Provider.max_output_tokens` (`kl4a/kl4a/llm_provider.py:89-93`).
    fn max_output_tokens(&self) -> i64 {
        let raw = self.value("max_output_tokens", "");
        if raw.is_empty() {
            return DEFAULT_MAX_OUTPUT_TOKENS;
        }
        raw.parse::<i64>().unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS)
    }

    /// Port of `Provider.timeout_seconds` (`kl4a/kl4a/llm_provider.py:95-99`).
    fn timeout_seconds(&self) -> u64 {
        let raw = self.value("timeout_seconds", "");
        if raw.is_empty() {
            return DEFAULT_TIMEOUT_SECONDS;
        }
        raw.parse::<u64>().unwrap_or(DEFAULT_TIMEOUT_SECONDS)
    }

    fn url(&self) -> Result<String, ProviderError>;
    fn headers(&self) -> Result<HashMap<String, String>, ProviderError>;
    fn body(&self, messages: &[Message]) -> Result<Value, ProviderError>;
    fn extract_text(&self, payload: &Value) -> Result<String, ProviderError>;

    /// Port of `Provider.complete` (`kl4a/kl4a/llm_provider.py:122-126`).
    fn complete(&self, messages: &[Message]) -> Result<String, ProviderError> {
        let payload = http_post_json(
            &self.url()?,
            &self.body(messages)?,
            &self.headers()?,
            self.timeout_seconds(),
            self.meta().label,
        )?;
        self.extract_text(&payload)
    }
}

/// Static per-provider metadata mirroring `Provider`'s Python class
/// attributes (`id`, `label`, `required_fields`, `env_names`, `summary`,
/// `field_guide`).
pub struct ProviderMeta {
    pub id: &'static str,
    pub label: &'static str,
    pub model_field_label: &'static str,
    pub required_fields: &'static [&'static str],
    pub env_names: &'static [(&'static str, &'static [&'static str])],
    pub summary: &'static str,
}

impl ProviderMeta {
    pub fn env_names_for(&self, field: &str) -> &'static [&'static str] {
        self.env_names
            .iter()
            .find(|(k, _)| *k == field)
            .map(|(_, v)| *v)
            .unwrap_or(&[])
    }
}

/// Port of `kl4a.kl4a.llm_provider.AzureResponsesProvider`
/// (`kl4a/kl4a/llm_provider.py:129-214`).
pub struct AzureResponsesProvider {
    pub settings: HashMap<String, String>,
}

pub static AZURE_RESPONSES_META: ProviderMeta = ProviderMeta {
    id: "azure-responses",
    label: "Azure OpenAI (Responses API)",
    model_field_label: "Deployment",
    required_fields: &["base_url", "api_key", "deployment"],
    env_names: &[
        ("base_url", &["AZURE_OPENAI_BASE_URL", "AZURE_OPENAI_ENDPOINT", "OPENAI_BASE_URL"]),
        ("api_key", &["AZURE_OPENAI_API_KEY"]),
        ("deployment", &["AZURE_OPENAI_DEPLOYMENT"]),
        ("max_output_tokens", &["AZURE_OPENAI_MAX_OUTPUT_TOKENS"]),
        ("timeout_seconds", &["AZURE_OPENAI_TIMEOUT_SECONDS"]),
        ("reasoning_effort", &["AZURE_OPENAI_REASONING_EFFORT"]),
    ],
    summary: "An Azure OpenAI resource you have deployed a model into.",
};

impl Provider for AzureResponsesProvider {
    fn meta(&self) -> &'static ProviderMeta {
        &AZURE_RESPONSES_META
    }
    fn settings(&self) -> &HashMap<String, String> {
        &self.settings
    }

    fn url(&self) -> Result<String, ProviderError> {
        Ok(format!("{}/responses", self.require("base_url")?.trim_end_matches('/')))
    }

    fn headers(&self) -> Result<HashMap<String, String>, ProviderError> {
        let mut h = HashMap::new();
        h.insert("Content-Type".to_string(), "application/json".to_string());
        h.insert("api-key".to_string(), self.require("api_key")?);
        Ok(h)
    }

    fn body(&self, messages: &[Message]) -> Result<Value, ProviderError> {
        let (system, user) = split_messages(messages);
        let mut body = Map::new();
        body.insert("model".to_string(), Value::String(self.require("deployment")?));
        body.insert("instructions".to_string(), Value::String(system));
        body.insert("input".to_string(), Value::String(user));
        body.insert(
            "max_output_tokens".to_string(),
            Value::Number(self.max_output_tokens().into()),
        );
        let effort = self.value("reasoning_effort", "");
        if !effort.is_empty() {
            let mut reasoning = Map::new();
            reasoning.insert("effort".to_string(), Value::String(effort));
            body.insert("reasoning".to_string(), Value::Object(reasoning));
        }
        Ok(Value::Object(body))
    }

    fn extract_text(&self, payload: &Value) -> Result<String, ProviderError> {
        if payload.get("status").and_then(|v| v.as_str()) == Some("incomplete") {
            let details = payload.get("incomplete_details").cloned().unwrap_or(Value::Null);
            let usage = payload.get("usage").cloned().unwrap_or(Value::Null);
            let reason = details.get("reason").and_then(|v| v.as_str()).unwrap_or("unknown");
            let total = usage.get("total_tokens");
            let output = usage.get("output_tokens");
            return Err(ProviderError::new(format!(
                "response incomplete (reason={reason}, total_tokens={}, output_tokens={}). \
                 Raise the max output tokens setting, or lower the reasoning effort.",
                total.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
                output.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
            )));
        }
        if let Some(text) = payload.get("output_text").and_then(|v| v.as_str()) {
            if !text.trim().is_empty() {
                return Ok(text.to_string());
            }
        }
        let mut chunks = Vec::new();
        if let Some(output) = payload.get("output").and_then(|v| v.as_array()) {
            for item in output {
                let Some(content) = item.get("content").and_then(|v| v.as_array()) else { continue };
                for c in content {
                    let kind = c.get("type").and_then(|v| v.as_str());
                    if matches!(kind, Some("output_text") | Some("text")) {
                        if let Some(text) = c.get("text").and_then(|v| v.as_str()) {
                            chunks.push(text.to_string());
                        }
                    }
                }
            }
        }
        if !chunks.is_empty() {
            return Ok(chunks.join("\n"));
        }
        Err(ProviderError::new(format!("unexpected response shape: {payload}")))
    }
}

/// Port of `kl4a.kl4a.llm_provider.OpenAICompatibleProvider`
/// (`kl4a/kl4a/llm_provider.py:217-291`).
pub struct OpenAICompatibleProvider {
    pub settings: HashMap<String, String>,
}

pub static OPENAI_COMPATIBLE_META: ProviderMeta = ProviderMeta {
    id: "openai-compatible",
    label: "OpenAI-compatible (OpenAI, Ollama, vLLM, gateways)",
    model_field_label: "Model",
    required_fields: &["base_url", "deployment"],
    env_names: &[
        ("base_url", &["OPENAI_BASE_URL"]),
        ("api_key", &["OPENAI_API_KEY"]),
        ("deployment", &["OPENAI_MODEL"]),
        ("max_output_tokens", &["OPENAI_MAX_OUTPUT_TOKENS"]),
        ("timeout_seconds", &["OPENAI_TIMEOUT_SECONDS"]),
    ],
    summary: "OpenAI itself, or anything that speaks its API: Ollama, vLLM, LiteLLM, OpenRouter, a company gateway.",
};

impl Provider for OpenAICompatibleProvider {
    fn meta(&self) -> &'static ProviderMeta {
        &OPENAI_COMPATIBLE_META
    }
    fn settings(&self) -> &HashMap<String, String> {
        &self.settings
    }

    fn url(&self) -> Result<String, ProviderError> {
        Ok(format!("{}/chat/completions", self.require("base_url")?.trim_end_matches('/')))
    }

    fn headers(&self) -> Result<HashMap<String, String>, ProviderError> {
        let mut h = HashMap::new();
        h.insert("Content-Type".to_string(), "application/json".to_string());
        let key = self.value("api_key", "");
        if !key.is_empty() {
            h.insert("Authorization".to_string(), format!("Bearer {key}"));
        }
        Ok(h)
    }

    fn body(&self, messages: &[Message]) -> Result<Value, ProviderError> {
        let msgs: Vec<Value> = messages
            .iter()
            .filter(|m| {
                m.get("content")
                    .and_then(|c| c.as_str())
                    .map(|c| !c.is_empty())
                    .unwrap_or(false)
            })
            .map(|m| {
                let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("user");
                let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let mut obj = Map::new();
                obj.insert("role".to_string(), Value::String(role.to_string()));
                obj.insert("content".to_string(), Value::String(content.to_string()));
                Value::Object(obj)
            })
            .collect();
        let mut body = Map::new();
        body.insert("model".to_string(), Value::String(self.require("deployment")?));
        body.insert("messages".to_string(), Value::Array(msgs));
        body.insert("max_tokens".to_string(), Value::Number(self.max_output_tokens().into()));
        Ok(Value::Object(body))
    }

    fn extract_text(&self, payload: &Value) -> Result<String, ProviderError> {
        if let Some(choices) = payload.get("choices").and_then(|v| v.as_array()) {
            if let Some(choice) = choices.first().and_then(|v| v.as_object()) {
                if choice.get("finish_reason").and_then(|v| v.as_str()) == Some("length") {
                    return Err(ProviderError::new(
                        "response was truncated by the model's output limit. \
                         Raise the max output tokens setting.",
                    ));
                }
                if let Some(content) = choice
                    .get("message")
                    .and_then(|v| v.as_object())
                    .and_then(|m| m.get("content"))
                    .and_then(|v| v.as_str())
                {
                    if !content.trim().is_empty() {
                        return Ok(content.to_string());
                    }
                }
            }
        }
        Err(ProviderError::new(format!("unexpected response shape: {payload}")))
    }
}

/// Port of `kl4a.kl4a.llm_provider.AnthropicProvider`
/// (`kl4a/kl4a/llm_provider.py:294-395`).
pub struct AnthropicProvider {
    pub settings: HashMap<String, String>,
}

const ANTHROPIC_DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_API_VERSION: &str = "2023-06-01";

pub static ANTHROPIC_META: ProviderMeta = ProviderMeta {
    id: "anthropic",
    label: "Anthropic (Messages API)",
    model_field_label: "Model",
    required_fields: &["api_key", "deployment"],
    env_names: &[
        ("base_url", &["ANTHROPIC_BASE_URL"]),
        ("api_key", &["ANTHROPIC_API_KEY"]),
        ("deployment", &["ANTHROPIC_MODEL"]),
        ("max_output_tokens", &["ANTHROPIC_MAX_OUTPUT_TOKENS"]),
        ("timeout_seconds", &["ANTHROPIC_TIMEOUT_SECONDS"]),
    ],
    summary: "Anthropic's own API. Two fields: a key and a model.",
};

impl Provider for AnthropicProvider {
    fn meta(&self) -> &'static ProviderMeta {
        &ANTHROPIC_META
    }
    fn settings(&self) -> &HashMap<String, String> {
        &self.settings
    }

    fn value(&self, field: &str, default: &str) -> String {
        if field == "base_url" {
            return self
                .settings()
                .get(field)
                .filter(|v| !v.is_empty())
                .cloned()
                .unwrap_or_else(|| ANTHROPIC_DEFAULT_BASE_URL.to_string())
                .trim()
                .to_string();
        }
        self.settings()
            .get(field)
            .filter(|v| !v.is_empty())
            .cloned()
            .unwrap_or_else(|| default.to_string())
            .trim()
            .to_string()
    }

    fn url(&self) -> Result<String, ProviderError> {
        let base = self.value("base_url", "");
        let base = base.trim_end_matches('/');
        let base = if base.ends_with("/v1") {
            base.to_string()
        } else {
            format!("{base}/v1")
        };
        Ok(format!("{base}/messages"))
    }

    fn headers(&self) -> Result<HashMap<String, String>, ProviderError> {
        let mut h = HashMap::new();
        h.insert("Content-Type".to_string(), "application/json".to_string());
        h.insert("x-api-key".to_string(), self.require("api_key")?);
        h.insert("anthropic-version".to_string(), ANTHROPIC_API_VERSION.to_string());
        Ok(h)
    }

    fn body(&self, messages: &[Message]) -> Result<Value, ProviderError> {
        let (system, user) = split_messages(messages);
        let mut msg = Map::new();
        msg.insert("role".to_string(), Value::String("user".to_string()));
        msg.insert("content".to_string(), Value::String(user));
        let mut body = Map::new();
        body.insert("model".to_string(), Value::String(self.require("deployment")?));
        body.insert("max_tokens".to_string(), Value::Number(self.max_output_tokens().into()));
        body.insert("messages".to_string(), Value::Array(vec![Value::Object(msg)]));
        if !system.is_empty() {
            body.insert("system".to_string(), Value::String(system));
        }
        Ok(Value::Object(body))
    }

    fn extract_text(&self, payload: &Value) -> Result<String, ProviderError> {
        let stop = payload.get("stop_reason").and_then(|v| v.as_str());
        if stop == Some("max_tokens") {
            return Err(ProviderError::new(
                "response was truncated by the output limit. Raise the max output tokens setting.",
            ));
        }
        if stop == Some("refusal") {
            let details = payload.get("stop_details").cloned().unwrap_or(Value::Null);
            let category = details.get("category").and_then(|v| v.as_str()).unwrap_or("unspecified");
            return Err(ProviderError::new(format!(
                "the model declined this request (category={category}). \
                 This is a safety refusal, not a configuration problem."
            )));
        }
        let mut chunks = Vec::new();
        if let Some(content) = payload.get("content").and_then(|v| v.as_array()) {
            for block in content {
                let is_text = block.get("type").and_then(|v| v.as_str()) == Some("text");
                if is_text {
                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                        chunks.push(text.to_string());
                    }
                }
            }
        }
        if !chunks.is_empty() {
            return Ok(chunks.join("\n"));
        }
        Err(ProviderError::new(format!("unexpected response shape: {payload}")))
    }
}

/// Port of `kl4a.kl4a.llm_provider.DEFAULT_PROVIDER_ID`
/// (`kl4a/kl4a/llm_provider.py:420`): `DEFAULT_PROVIDER_ID = AzureResponsesProvider.id`.
pub const DEFAULT_PROVIDER_ID: &str = "azure-responses";

/// Port of `kl4a.kl4a.llm_provider.PROVIDERS` (`kl4a/kl4a/llm_provider.py:398-402`):
/// the ordered list of known provider ids (Rust has no direct analogue of a
/// `dict[str, type[Provider]]` without dynamic dispatch boilerplate — see
/// [`build_provider`], which switches on the id directly instead).
pub const PROVIDER_IDS: [&str; 3] = ["azure-responses", "openai-compatible", "anthropic"];

/// Port of `kl4a.kl4a.llm_provider.PROVIDER_ALIASES` (`kl4a/kl4a/llm_provider.py:406-412`).
pub fn provider_aliases(alias: &str) -> Option<&'static str> {
    match alias {
        "azure-llm" | "azure" | "llm" => Some("azure-responses"),
        "openai" => Some("openai-compatible"),
        "claude" => Some("anthropic"),
        _ => None,
    }
}

/// Port of `kl4a.kl4a.llm_provider.resolve_provider_id` (`kl4a/kl4a/llm_provider.py:423-429`).
///
/// ```python
/// def resolve_provider_id(name: str | None) -> str:
///     key = (name or "").strip().lower()
///     if not key:
///         return DEFAULT_PROVIDER_ID
///     if key in PROVIDERS:
///         return key
///     return PROVIDER_ALIASES.get(key, DEFAULT_PROVIDER_ID)
/// ```
pub fn resolve_provider_id(name: Option<&str>) -> String {
    let key = name.unwrap_or("").trim().to_lowercase();
    if key.is_empty() {
        return DEFAULT_PROVIDER_ID.to_string();
    }
    if PROVIDER_IDS.contains(&key.as_str()) {
        return key;
    }
    provider_aliases(&key).unwrap_or(DEFAULT_PROVIDER_ID).to_string()
}

/// Boxed-trait-object stand-in for Python's `type[Provider]` /
/// `Provider` instance returned by `build_provider`/`provider_class`.
pub fn build_provider(settings: &HashMap<String, String>) -> Box<dyn Provider> {
    let id = resolve_provider_id(settings.get("provider").map(|s| s.as_str()));
    let settings = settings.clone();
    match id.as_str() {
        "openai-compatible" => Box::new(OpenAICompatibleProvider { settings }),
        "anthropic" => Box::new(AnthropicProvider { settings }),
        _ => Box::new(AzureResponsesProvider { settings }),
    }
}

/// Port of `kl4a.kl4a.llm_provider.parse_author_response`
/// (`kl4a/kl4a/llm_provider.py:471-483`).
///
/// ```python
/// def parse_author_response(text: str) -> dict[str, Any]:
///     try:
///         data = json.loads(text)
///     except json.JSONDecodeError as exc:
///         raise ValueError(f"LLM author response must be JSON: {exc}") from exc
///     if not isinstance(data, dict):
///         raise ValueError("LLM author response must be a JSON object")
///     return data
/// ```
pub fn parse_author_response(text: &str) -> anyhow::Result<Value> {
    let data: Value = serde_json::from_str(text)
        .map_err(|exc| anyhow::anyhow!("LLM author response must be JSON: {exc}"))?;
    if !data.is_object() {
        anyhow::bail!("LLM author response must be a JSON object");
    }
    Ok(data)
}
