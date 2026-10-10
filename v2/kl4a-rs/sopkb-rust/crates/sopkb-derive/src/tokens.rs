//! `meaningful_tokens` / `knowledge_haystack`. docs/port/port-mapping-d-agent-mcp.md §3.5-3.6.

use crate::constants::STOPWORDS;
use std::collections::HashSet;

/// Regex `[a-zA-Z][a-zA-Z0-9]+` over the lowercased value (non-overlapping, greedy),
/// filtered to length > 2 and not a stopword. A lone letter with no trailing
/// alphanumeric char never matches at all (the `+` requires at least one more char) --
/// implemented as a hand-rolled scan rather than pulling in `regex` for one pattern.
pub fn meaningful_tokens(value: &str) -> HashSet<String> {
    let lowered = value.to_lowercase();
    let chars: Vec<char> = lowered.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_alphabetic() {
            let start = i;
            i += 1;
            let mut trailing = 0;
            while i < chars.len() && chars[i].is_ascii_alphanumeric() {
                i += 1;
                trailing += 1;
            }
            if trailing >= 1 {
                tokens.push(chars[start..i].iter().collect::<String>());
            }
        } else {
            i += 1;
        }
    }
    tokens.into_iter().filter(|t| t.len() > 2 && !STOPWORDS.contains(&t.as_str())).collect()
}

/// `join([subject, predicate, object, source_text], " ")` then lowercase. Missing keys
/// default to `""` (defaulted lookup, not a hard error here).
pub fn knowledge_haystack(item: &serde_json::Value) -> String {
    let get = |k: &str| item.get(k).and_then(|v| v.as_str()).unwrap_or("");
    format!("{} {} {} {}", get("subject"), get("predicate"), get("object"), get("source_text")).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_require_ascii_letter_start_and_min_length_3() {
        let t = meaningful_tokens("2024 5mg 7 glp1 follow-up patient scenario identity confirmed");
        assert!(!t.contains("mg")); // len 2, dropped
        assert!(t.contains("glp1"));
        assert!(t.contains("follow"));
        assert!(!t.contains("up")); // len 2, dropped
        assert!(!t.contains("patient")); // stopword
        assert!(!t.contains("scenario")); // stopword
        assert!(t.contains("identity"));
        assert!(t.contains("confirmed"));
    }

    #[test]
    fn tokens_non_ascii_acts_as_separator() {
        // 'é' is not ASCII-alphabetic, so it splits "café" into "caf" (kept, len 3) --
        // the 'é' itself contributes no token.
        let t = meaningful_tokens("café checklist");
        assert!(t.contains("caf"));
        assert!(t.contains("checklist"));
    }

    #[test]
    fn haystack_defaults_missing_keys_to_empty() {
        let item = serde_json::json!({"subject": "Eligibility"});
        assert_eq!(knowledge_haystack(&item), "eligibility   ");
    }
}
