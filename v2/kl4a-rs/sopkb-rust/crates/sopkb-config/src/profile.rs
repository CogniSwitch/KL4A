//! `ModelProfile` and the `PROFILE_FIELDS` ordered field list. docs/port/port-mapping-b-mining-settings.md §1.1.

/// All 11 fields are strings in storage; numbers (`max_output_tokens`, `timeout_seconds`)
/// are stored as strings and parsed at use site. Blank string means "unset" everywhere.
pub const PROFILE_FIELDS: &[&str] = &[
    "id",
    "name",
    "base_url",
    "api_key",
    "auth_style",
    "model",
    "max_output_tokens",
    "timeout_seconds",
    "reasoning_effort",
    "mining_prompt",
    "chat_prompt",
];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub auth_style: String,
    pub model: String,
    pub max_output_tokens: String,
    pub timeout_seconds: String,
    pub reasoning_effort: String,
    pub mining_prompt: String,
    pub chat_prompt: String,
}

impl ModelProfile {
    pub fn get(&self, field: &str) -> Option<&str> {
        match field {
            "id" => Some(&self.id),
            "name" => Some(&self.name),
            "base_url" => Some(&self.base_url),
            "api_key" => Some(&self.api_key),
            "auth_style" => Some(&self.auth_style),
            "model" => Some(&self.model),
            "max_output_tokens" => Some(&self.max_output_tokens),
            "timeout_seconds" => Some(&self.timeout_seconds),
            "reasoning_effort" => Some(&self.reasoning_effort),
            "mining_prompt" => Some(&self.mining_prompt),
            "chat_prompt" => Some(&self.chat_prompt),
            _ => None,
        }
    }

    /// `{field: to_string(profile.get(field, "")) for field in PROFILE_FIELDS}` --
    /// stringified but NOT stripped; `serde_json::Value::Null` becomes the literal
    /// string `"None"` (matching Python's `str(None)`), any other JSON scalar is
    /// stringified via its natural `str()`/Display form.
    pub fn from_json_normalized(value: &serde_json::Value) -> Self {
        let get = |field: &str| -> String {
            match value.get(field) {
                None => String::new(),
                Some(serde_json::Value::Null) => "None".to_string(),
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
            }
        };
        ModelProfile {
            id: get("id"),
            name: get("name"),
            base_url: get("base_url"),
            api_key: get("api_key"),
            auth_style: get("auth_style"),
            model: get("model"),
            max_output_tokens: get("max_output_tokens"),
            timeout_seconds: get("timeout_seconds"),
            reasoning_effort: get("reasoning_effort"),
            mining_prompt: get("mining_prompt"),
            chat_prompt: get("chat_prompt"),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id, "name": self.name, "base_url": self.base_url, "api_key": self.api_key,
            "auth_style": self.auth_style, "model": self.model, "max_output_tokens": self.max_output_tokens,
            "timeout_seconds": self.timeout_seconds, "reasoning_effort": self.reasoning_effort,
            "mining_prompt": self.mining_prompt, "chat_prompt": self.chat_prompt,
        })
    }
}
