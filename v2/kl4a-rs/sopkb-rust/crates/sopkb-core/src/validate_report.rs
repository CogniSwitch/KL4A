//! `render_validation_report` -- pulled forward into Phase 1 because it needs no
//! other subsystem (docs/port/PORT_PLAN.md §6.2 Done-when #3; full `validate_bundle`
//! integration is Phase 3, docs/port/port-mapping-a-core-data.md §3.6).

/// Exact layout: no blank line between the two counts; the trailing empty line
/// produces exactly one terminating newline when joined with `\n`.
pub fn render_validation_report(errors: &[String], warnings: &[String]) -> String {
    let mut lines: Vec<String> = vec!["# Validation Report".to_string(), String::new()];
    lines.push(format!("Errors: {}", errors.len()));
    lines.push(format!("Warnings: {}", warnings.len()));
    lines.push(String::new());
    lines.push("## Errors".to_string());
    if errors.is_empty() {
        lines.push("- None".to_string());
    } else {
        lines.extend(errors.iter().map(|e| format!("- {e}")));
    }
    lines.push(String::new());
    lines.push("## Warnings".to_string());
    if warnings.is_empty() {
        lines.push("- None".to_string());
    } else {
        lines.extend(warnings.iter().map(|w| format!("- {w}")));
    }
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v3_golden_clean_bundle() {
        let out = render_validation_report(&[], &[]);
        assert_eq!(
            out,
            "# Validation Report\n\nErrors: 0\nWarnings: 0\n\n## Errors\n- None\n\n## Warnings\n- None\n"
        );
    }

    #[test]
    fn v3_golden_with_entries() {
        let out = render_validation_report(
            &["missing required directory: .sopkb/cache".to_string()],
            &["missing .sopkb/inventory.json".to_string()],
        );
        assert_eq!(
            out,
            "# Validation Report\n\nErrors: 1\nWarnings: 1\n\n## Errors\n- missing required directory: .sopkb/cache\n\n## Warnings\n- missing .sopkb/inventory.json\n"
        );
    }
}
