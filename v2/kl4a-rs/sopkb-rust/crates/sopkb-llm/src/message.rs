//! The `{role, content}` message list shape shared by `okf_author.build_author_messages`
//! and `agent_chat.build_agent_messages` (built in `sopkb-mining`/`sopkb-agent`
//! respectively -- this crate only flattens and sends).

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Message { role: "system".to_string(), content: content.into() }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Message { role: "user".to_string(), content: content.into() }
    }
}

/// `(system_content, user_content)` = each role's contents joined with `"\n\n"`, in
/// list order. Mirrors both `call_azure_responses` implementations' first two lines
/// exactly (docs/port/port-mapping-b-mining-settings.md §3.2.4,
/// docs/port/port-mapping-d-agent-mcp.md §5.5).
pub fn flatten_messages(messages: &[Message]) -> (String, String) {
    let system_content = messages.iter().filter(|m| m.role == "system").map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n\n");
    let user_content = messages.iter().filter(|m| m.role == "user").map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n\n");
    (system_content, user_content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_by_role_in_order_ignoring_other_roles() {
        let messages = vec![
            Message::system("sys1"),
            Message::user("user1"),
            Message { role: "developer".into(), content: "ignored".into() },
            Message::system("sys2"),
            Message::user("user2"),
        ];
        let (system, user) = flatten_messages(&messages);
        assert_eq!(system, "sys1\n\nsys2");
        assert_eq!(user, "user1\n\nuser2");
    }

    #[test]
    fn empty_message_list_yields_empty_strings() {
        let (system, user) = flatten_messages(&[]);
        assert_eq!(system, "");
        assert_eq!(user, "");
    }
}
