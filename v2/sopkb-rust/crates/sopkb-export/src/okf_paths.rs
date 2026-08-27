//! OKF document path/link helpers. docs/port/port-mapping-c-review-export.md §3.7.

use sopkb_derive::relations::{evidence_id_for_item, relation_id_for_item};

pub fn source_doc_path(source: &serde_json::Value) -> String {
    format!("sources/{}.md", source["id"].as_str().unwrap_or(""))
}

/// Works for BOTH a section record and a knowledge item: uses `id` when present and
/// `section-`-prefixed (a section record), else falls back to `section_id` (a
/// knowledge item, whose own `id` is `ki-...`).
pub fn section_doc_path(section_or_item: &serde_json::Value) -> String {
    let source_id = section_or_item["source_id"].as_str().unwrap_or("");
    let section_id = match section_or_item.get("id").and_then(|v| v.as_str()) {
        Some(id) if id.starts_with("section-") => id.to_string(),
        _ => section_or_item["section_id"].as_str().unwrap_or("").to_string(),
    };
    format!("sections/{source_id}/{section_id}.md")
}

pub fn knowledge_doc_name(item: &serde_json::Value) -> String {
    format!("{}.md", item["id"].as_str().unwrap_or(""))
}

pub fn knowledge_doc_path(item: &serde_json::Value) -> String {
    format!("knowledge/{}", knowledge_doc_name(item))
}

pub fn evidence_doc_path(item: &serde_json::Value) -> String {
    format!("evidence/{}.md", evidence_id_for_item(item))
}

pub fn relation_doc_path(item: &serde_json::Value) -> String {
    format!("relations/{}.md", relation_id_for_item(item))
}

pub fn rule_doc_path(rule: &serde_json::Value) -> String {
    format!("rules/{}.md", rule["id"].as_str().unwrap_or(""))
}

pub fn concept_doc_path(concept_id: &str) -> String {
    format!("concepts/{concept_id}.md")
}

/// `str(PurePosixPath(from_rel_path).parent)`, with `"."` normalized to `""` by the
/// caller (`relative_link` does that itself, matching Python's `if start == ".":
/// start = ""` followed by `start or "."`).
fn posix_parent(p: &str) -> String {
    match p.rfind('/') {
        Some(idx) => {
            let head = &p[..idx];
            if head.is_empty() { "/".to_string() } else { head.to_string() }
        }
        None => ".".to_string(),
    }
}

/// `posixpath.normpath`'s component-collapsing rule for a path with no leading `/`
/// (our domain never has absolute or `..`-containing inputs, but this is implemented
/// generally rather than assumed away).
fn normpath_components(path: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for comp in path.split('/') {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp != ".." {
            out.push(comp.to_string());
        } else if out.is_empty() || out.last().map(String::as_str) == Some("..") {
            out.push("..".to_string());
        } else {
            out.pop();
        }
    }
    out
}

/// `posixpath.relpath(path, start)`. Both inputs are always relative (never `/`-
/// prefixed) in this codebase's domain, so the `os.getcwd()` prefix `abspath` would
/// normally add is a constant shared by both sides and cancels out of the
/// common-prefix computation -- this implements the resulting relationship directly
/// rather than needing a real filesystem cwd.
fn posix_relpath(path: &str, start: &str) -> String {
    let start_list = normpath_components(start);
    let path_list = normpath_components(path);
    let common = start_list.iter().zip(path_list.iter()).take_while(|(a, b)| a == b).count();
    let mut rel: Vec<String> = Vec::with_capacity(start_list.len() - common + path_list.len() - common);
    for _ in 0..(start_list.len() - common) {
        rel.push("..".to_string());
    }
    rel.extend(path_list[common..].iter().cloned());
    if rel.is_empty() {
        ".".to_string()
    } else {
        rel.join("/")
    }
}

/// `relative_link(from_rel, to_rel)`: `posixpath.relpath(to_rel, start=parent(from_rel))`.
pub fn relative_link(from_rel_path: &str, to_rel_path: &str) -> String {
    let parent = posix_parent(from_rel_path);
    let start = if parent == "." { "." } else { parent.as_str() };
    posix_relpath(to_rel_path, start)
}

pub fn bullet_link(from_rel_path: &str, to_rel_path: &str, text: &str) -> String {
    format!("- [{text}]({})", relative_link(from_rel_path, to_rel_path))
}

/// `source.get("original_path") or source.get("path")` (truthy -- empty string
/// counts as absent too), relativized from the source doc's own path; falls back to
/// the bare source id when neither is present.
pub fn source_resource_for_okf(source: &serde_json::Value) -> String {
    let path = source
        .get("original_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| source.get("path").and_then(|v| v.as_str()).filter(|s| !s.is_empty()));
    match path {
        Some(p) => relative_link(&source_doc_path(source), p),
        None => source["id"].as_str().unwrap_or("").to_string(),
    }
}

/// `section_id.rsplit("-", 1)[-1]`; `int(suffix) if suffix.isdigit() else None`.
pub fn ordinal_from_section_id(section_id: &str) -> Option<u64> {
    let suffix = section_id.rsplit('-').next().unwrap_or("");
    if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
        suffix.parse::<u64>().ok()
    } else {
        None
    }
}

/// `values if values else [placeholder]`.
pub fn or_placeholder(values: Vec<String>, placeholder: &str) -> Vec<String> {
    if values.is_empty() {
        vec![placeholder.to_string()]
    } else {
        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_link_worked_examples_from_spec() {
        assert_eq!(relative_link("knowledge/x.md", "evidence/y.md"), "../evidence/y.md");
        assert_eq!(relative_link("concepts/index.md", "concepts/c.md"), "c.md");
        assert_eq!(relative_link("sections/index.md", "sections/src/sec.md"), "src/sec.md");
    }

    #[test]
    fn relative_link_same_directory() {
        assert_eq!(relative_link("knowledge/a.md", "knowledge/b.md"), "b.md");
    }

    #[test]
    fn relative_link_top_level_to_nested() {
        assert_eq!(relative_link("index.md", "sources/index.md"), "sources/index.md");
    }

    #[test]
    fn section_doc_path_for_section_record_vs_knowledge_item() {
        let section = serde_json::json!({"id": "section-src-001", "source_id": "src"});
        assert_eq!(section_doc_path(&section), "sections/src/section-src-001.md");
        let item = serde_json::json!({"id": "ki-src-000001", "source_id": "src", "section_id": "section-src-001"});
        assert_eq!(section_doc_path(&item), "sections/src/section-src-001.md");
    }

    #[test]
    fn ordinal_from_section_id_digit_and_non_digit_suffix() {
        assert_eq!(ordinal_from_section_id("section-src-001"), Some(1));
        assert_eq!(ordinal_from_section_id("section-src-abc"), None);
    }

    #[test]
    fn source_resource_prefers_original_path_over_path() {
        // source_doc_path(source) = "sources/src.md"; relative_link from there to
        // "sources/originals/src.md" shares the "sources" parent, so it resolves
        // WITHIN that directory rather than escaping it.
        let source = serde_json::json!({"id": "src", "original_path": "sources/originals/src.md"});
        assert_eq!(source_resource_for_okf(&source), "originals/src.md");
        let no_path = serde_json::json!({"id": "src"});
        assert_eq!(source_resource_for_okf(&no_path), "src");
    }
}
