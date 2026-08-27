//! Port of `tools/sopkb/sopkb/okf_writer.py`. docs/port/port-mapping-b-mining-settings.md
//! §3.3. `OKFDocumentError` in Python subclasses `ValueError` (P-M27, so callers
//! catching `ValueError` treat OKF validation failures the same as author-response
//! failures) -- represented here as `SopkbError::Value`, matching every other crate's
//! convention for that error family.
//!
//! `OKFDocument.parse` (the round-trip reader) is P-M28/F17: dead code with no caller
//! in the real pipeline. Dropped, not ported, per PORT_PLAN.md's recommendation.

use crate::ordered_json::{ordered_json_to_yaml, OrderedJson};
use sopkb_core::error::{Result, SopkbError};
use sopkb_core::store::write_text_native;
use sopkb_fmt::{emit_yaml, OrderedMap, YamlValue};
use std::path::Path;

pub const REQUIRED_FRONTMATTER_KEYS: &[&str] = &["type"];
pub const RESERVED_FILENAMES: &[&str] = &["index.md", "log.md"];

/// Result of preparing (validating + serializing) an authored document, before it is
/// written to disk. Kept separate from the write step so `okf_author::mine_with_author`
/// can stage every section's authored documents in memory and only commit them to
/// `.sopkb/authored_okf/` once the whole mining run succeeds (P-M11 FIX -- see
/// DEVIATIONS.md).
#[derive(Debug, Clone)]
pub struct PreparedDoc {
    /// POSIX path components relative to `okf_root`, with the final component already
    /// suffixed `.md`. Kept as parts (not a `Path`/`String`) so building the on-disk
    /// target (native separators, via repeated `PathBuf::push`) and the POSIX `"path"`
    /// return value are both trivial, without parsing a string that might contain
    /// platform-specific separators back into components.
    pub rel_parts: Vec<String>,
    pub text: String,
    pub bytes: usize,
}

impl PreparedDoc {
    pub fn posix_path(&self) -> String {
        self.rel_parts.join("/")
    }

    pub fn to_write_entry(&self) -> serde_json::Value {
        serde_json::json!({ "path": self.posix_path(), "bytes": self.bytes })
    }
}

#[derive(Debug)]
pub struct WriteResult {
    pub path: String,
    pub bytes: usize,
}

/// `concept_id_to_path`: normalizes `\` to `/`, strips one trailing `.md` (case
/// sensitive -- `"x.MD"` is untouched and ends up `"x.MD.md"`), then validates the
/// result as a `PurePosixPath`.
///
/// P-M24 FIX: Python's `PurePosixPath("C:x").is_absolute()` is `False` (POSIX
/// semantics only recognize a leading `/`), so a Windows drive-letter-shaped component
/// passes straight through unvalidated in the original. On a POSIX host that's inert
/// (no such thing as a `C:` drive), but this port runs on Windows, where
/// `okf_root.join(["C:", "x.md"])`-style construction would let a crafted concept id
/// escape `okf_root` onto an arbitrary drive. Reject any component shaped like a drive
/// letter (`[A-Za-z]:...`), not just a leading one, since `Path::push` treats a
/// `X:`-prefixed component as a new drive root regardless of its position in a
/// sequence of pushes.
pub fn concept_id_to_path(concept_id: &str) -> Result<Vec<String>> {
    let trimmed = concept_id.trim();
    let normalized_slashes = trimmed.replace('\\', "/");
    let normalized = normalized_slashes.strip_suffix(".md").unwrap_or(&normalized_slashes);

    let is_absolute = normalized.starts_with('/');

    // PurePosixPath.parts: split on '/', drop empty segments (collapses "//" and a
    // trailing "/") and single "." segments (pathlib normalizes those away); a bare
    // "" or "." therefore yields zero parts, matching Python's `PurePosixPath("").parts
    // == () ` / `PurePosixPath(".").parts == ()`.
    let parts: Vec<&str> = normalized.split('/').filter(|p| !p.is_empty() && *p != ".").collect();

    if is_absolute || parts.contains(&"..") || parts.is_empty() {
        return Err(SopkbError::Value(format!("invalid concept id: {concept_id}")));
    }
    if parts.iter().any(|p| looks_like_windows_drive(p)) {
        return Err(SopkbError::Value(format!("invalid concept id: {concept_id}")));
    }

    let mut result: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
    let last_idx = result.len() - 1;
    let filename = format!("{}.md", result[last_idx]);
    if RESERVED_FILENAMES.contains(&filename.as_str()) {
        return Err(SopkbError::Value(format!("reserved filename cannot be authored as a concept: {filename}")));
    }
    result[last_idx] = filename;
    Ok(result)
}

fn looks_like_windows_drive(part: &str) -> bool {
    let bytes = part.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Today's UTC date as `YYYY-MM-DD` (no time component). Reuses
/// `sopkb_core::store::utc_now()`'s `YYYY-MM-DDTHH:MM:SSZ` output rather than adding a
/// direct `chrono` dependency to this crate.
fn today_utc_date() -> String {
    sopkb_core::store::utc_now()[..10].to_string()
}

/// `OKFDocument.serialize`/`.validate`. `frontmatter` is always a mapping by
/// construction (every call site into this type either received an `OrderedJson`
/// already known to be an `Object`, or built one directly) -- unlike Python, where
/// `frontmatter: dict[str, Any]` is a runtime-unenforced type hint, this is enforced by
/// the type itself.
pub struct OkfDocument {
    pub frontmatter: OrderedMap<OrderedJson>,
    pub body: String,
}

impl OkfDocument {
    pub fn serialize(&self) -> Result<String> {
        self.validate()?;
        let mut yaml_map = OrderedMap::new();
        for (k, v) in self.frontmatter.iter() {
            yaml_map.insert(k, ordered_json_to_yaml(v));
        }
        let frontmatter_text = emit_yaml(&YamlValue::Mapping(yaml_map));
        // Python `.rstrip()`: strips ALL trailing whitespace (not just the final `\n`).
        let frontmatter_text = frontmatter_text.trim_end();
        let body = if self.body.ends_with('\n') { self.body.clone() } else { format!("{}\n", self.body) };
        Ok(format!("---\n{frontmatter_text}\n---\n\n{body}"))
    }

    pub fn validate(&self) -> Result<()> {
        for key in REQUIRED_FRONTMATTER_KEYS {
            let truthy = self.frontmatter.get(key).map(|v| v.is_truthy()).unwrap_or(false);
            if !truthy {
                return Err(SopkbError::Value(format!("missing required frontmatter key: {key}")));
            }
        }

        // `sources = frontmatter.get("sources", [])`. Python's guard
        // (`if sources and not isinstance(sources, list): raise`) only fires for a
        // TRUTHY non-list; a falsy `sources` (absent, null, false, 0, "", {}) falls
        // through to `for source in sources`, which Python crashes on for the
        // non-iterable falsy values null/false/0 (P-M26a, one of the two raw crash
        // paths this function FIXes into a typed error) and silently no-ops for the
        // iterable-but-empty falsy values "" and {}. We treat every falsy value
        // uniformly as "no sources" (skip), matching Python's evident intent without
        // the crash and without changing behavior for any input that didn't already
        // crash.
        if let Some(sources_val) = self.frontmatter.get("sources") {
            if sources_val.is_truthy() {
                match sources_val {
                    OrderedJson::Array(items) => {
                        for source in items {
                            let ok = source
                                .as_object()
                                .map(|o| o.get("resource").map(|r| r.is_truthy()).unwrap_or(false))
                                .unwrap_or(false);
                            if !ok {
                                return Err(SopkbError::Value("each sources entry must include resource".to_string()));
                            }
                        }
                    }
                    _ => return Err(SopkbError::Value("sources must be a list when present".to_string())),
                }
            }
        }

        // `rule = (frontmatter.get("sopkb") or {}).get("rule")`. A present-but-null
        // "rule" key and an absent "rule" key are indistinguishable via `.get()`, same
        // as Python -- both fold into `OrderedJson::Null` below, then skipped.
        //
        // P-M26b (FIX): a truthy non-mapping `sopkb` value (e.g. `sopkb: "oops"`)
        // reaches `.get("rule")` in Python and raises a raw `AttributeError` since
        // strings have no `.get`. Converted to a typed validation error here instead.
        let rule_value: OrderedJson = match self.frontmatter.get("sopkb") {
            None => OrderedJson::Null,
            Some(sopkb_val) if !sopkb_val.is_truthy() => OrderedJson::Null,
            Some(sopkb_val) => match sopkb_val.as_object() {
                Some(sopkb_obj) => sopkb_obj.get("rule").cloned().unwrap_or(OrderedJson::Null),
                None => return Err(SopkbError::Value("frontmatter.sopkb must be a mapping".to_string())),
            },
        };
        if !matches!(rule_value, OrderedJson::Null) {
            validate_sopkb_rule(&rule_value)?;
        }
        Ok(())
    }
}

fn validate_sopkb_rule(rule: &OrderedJson) -> Result<()> {
    let rule_obj = match rule.as_object() {
        Some(o) => o,
        None => return Err(SopkbError::Value("sopkb.rule must be a mapping".to_string())),
    };
    let id_truthy = rule_obj.get("id").map(|v| v.is_truthy()).unwrap_or(false);
    if !id_truthy {
        return Err(SopkbError::Value("sopkb.rule.id is required".to_string()));
    }
    let obligation_obj = match rule_obj.get("obligation").and_then(|v| v.as_object()) {
        Some(o) => o,
        None => return Err(SopkbError::Value("sopkb.rule.obligation is required".to_string())),
    };
    for key in ["fact", "action", "label"] {
        let truthy = obligation_obj.get(key).map(|v| v.is_truthy()).unwrap_or(false);
        if !truthy {
            return Err(SopkbError::Value(format!("sopkb.rule.obligation.{key} is required")));
        }
    }

    if let Some(condition) = rule_obj.get("condition") {
        if !matches!(condition, OrderedJson::Null) {
            let condition_obj = match condition.as_object() {
                Some(o) => o,
                None => return Err(SopkbError::Value("sopkb.rule.condition must be a mapping".to_string())),
            };
            for key in ["fact", "operator", "label"] {
                let truthy = condition_obj.get(key).map(|v| v.is_truthy()).unwrap_or(false);
                if !truthy {
                    return Err(SopkbError::Value(format!("sopkb.rule.condition.{key} is required")));
                }
            }
        }
    }

    if let Some(otherwise) = rule_obj.get("otherwise") {
        if !matches!(otherwise, OrderedJson::Null) {
            let otherwise_obj = match otherwise.as_object() {
                Some(o) => o,
                None => return Err(SopkbError::Value("sopkb.rule.otherwise must be a mapping".to_string())),
            };
            if let Some(ar) = otherwise_obj.get("action_required") {
                if !matches!(ar, OrderedJson::Bool(_)) {
                    return Err(SopkbError::Value("sopkb.rule.otherwise.action_required must be boolean".to_string()));
                }
            }
        }
    }
    Ok(())
}

/// `write_concept_doc`'s validate+serialize half, without touching disk -- the seam
/// `okf_author::mine_with_author` uses to stage documents (P-M11 FIX).
pub fn prepare_concept_doc(
    concept_id: &str,
    frontmatter: &OrderedMap<OrderedJson>,
    body: &str,
    actor: &str,
) -> Result<PreparedDoc> {
    let rel_parts = concept_id_to_path(concept_id)?;
    let mut fm = frontmatter.clone();
    if !fm.contains_key("generated") {
        let mut generated = OrderedMap::new();
        generated.insert("actor", OrderedJson::String(actor.to_string()));
        generated.insert("date", OrderedJson::String(today_utc_date()));
        fm.insert("generated", OrderedJson::Object(generated));
    }
    let doc = OkfDocument { frontmatter: fm, body: body.to_string() };
    let text = doc.serialize()?;
    let bytes = text.len();
    Ok(PreparedDoc { rel_parts, text, bytes })
}

/// Writes an already-`prepare_concept_doc`d document to disk (full overwrite, no
/// backup, no locking -- matches Python exactly for this half).
pub fn write_prepared_doc(okf_root: &Path, prepared: &PreparedDoc) -> Result<WriteResult> {
    let mut target = okf_root.to_path_buf();
    for part in &prepared.rel_parts {
        target.push(part);
    }
    write_text_native(&target, &prepared.text)?;
    Ok(WriteResult { path: prepared.posix_path(), bytes: prepared.bytes })
}

/// `write_concept_doc`: validates, serializes, and writes in one call (used directly
/// by callers that don't need staging, and by unit tests).
pub fn write_concept_doc(
    okf_root: &Path,
    concept_id: &str,
    frontmatter: &OrderedMap<OrderedJson>,
    body: &str,
    actor: &str,
) -> Result<WriteResult> {
    let prepared = prepare_concept_doc(concept_id, frontmatter, body, actor)?;
    write_prepared_doc(okf_root, &prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fm(pairs: Vec<(&str, OrderedJson)>) -> OrderedMap<OrderedJson> {
        let mut m = OrderedMap::new();
        for (k, v) in pairs {
            m.insert(k, v);
        }
        m
    }

    fn obj(pairs: Vec<(&str, OrderedJson)>) -> OrderedJson {
        OrderedJson::Object(fm(pairs))
    }

    fn s(v: &str) -> OrderedJson {
        OrderedJson::String(v.to_string())
    }

    // --- concept_id_to_path -----------------------------------------------------

    #[test]
    fn concept_id_to_path_basic_and_md_suffix_handling() {
        assert_eq!(concept_id_to_path("rules/check-identity").unwrap(), vec!["rules", "check-identity.md"]);
        assert_eq!(concept_id_to_path("rules/check-identity.md").unwrap(), vec!["rules", "check-identity.md"]);
        // Suffix removal is case-sensitive: ".MD" is not treated as ".md".
        assert_eq!(concept_id_to_path("rules/x.MD").unwrap(), vec!["rules", "x.MD.md"]);
    }

    #[test]
    fn concept_id_to_path_backslashes_become_slashes() {
        assert_eq!(concept_id_to_path("rules\\nested\\doc").unwrap(), vec!["rules", "nested", "doc.md"]);
    }

    #[test]
    fn concept_id_to_path_rejects_absolute_dotdot_and_empty() {
        assert!(concept_id_to_path("/rules/x").is_err());
        assert!(concept_id_to_path("rules/../x").is_err());
        assert!(concept_id_to_path("a..b/x").is_ok(), "'..' only rejected as a WHOLE component");
        assert!(concept_id_to_path("").is_err());
        assert!(concept_id_to_path(".").is_err());
    }

    #[test]
    fn concept_id_to_path_rejects_reserved_filenames_at_any_depth() {
        let err = concept_id_to_path("index").unwrap_err().to_string();
        assert!(err.contains("reserved filename"), "{err}");
        assert!(concept_id_to_path("deep/nested/log").is_err());
        assert!(concept_id_to_path("deep/nested/log.md").is_err());
    }

    #[test]
    fn concept_id_to_path_p_m24_rejects_windows_drive_letter_traversal() {
        // Not absolute in POSIX semantics (no leading "/"), so Python's PurePosixPath
        // guard alone would let this through; the FIX rejects it explicitly since this
        // port runs on Windows, where a "C:"-shaped component would otherwise escape
        // okf_root.
        assert!(concept_id_to_path("C:x").is_err());
        assert!(concept_id_to_path("C:\\evil\\escape").is_err());
        assert!(concept_id_to_path("rules/C:/evil").is_err());
    }

    // --- OkfDocument::validate ---------------------------------------------------

    #[test]
    fn validate_requires_truthy_type() {
        let doc = OkfDocument { frontmatter: fm(vec![]), body: "x".into() };
        let err = doc.validate().unwrap_err().to_string();
        assert_eq!(err, "missing required frontmatter key: type");

        let doc = OkfDocument { frontmatter: fm(vec![("type", s(""))]), body: "x".into() };
        assert!(doc.validate().is_err());
    }

    #[test]
    fn validate_sources_entries_require_resource() {
        let doc = OkfDocument {
            frontmatter: fm(vec![("type", s("Doc")), ("sources", OrderedJson::Array(vec![obj(vec![("id", s("a"))])]))]),
            body: "x".into(),
        };
        let err = doc.validate().unwrap_err().to_string();
        assert_eq!(err, "each sources entry must include resource");
    }

    #[test]
    fn validate_sources_null_is_treated_as_no_sources_not_a_crash() {
        // P-M26a: Python crashes with a raw TypeError iterating `None`; this must
        // succeed instead, treating null the same as an absent/empty sources list.
        let doc = OkfDocument { frontmatter: fm(vec![("type", s("Doc")), ("sources", OrderedJson::Null)]), body: "x".into() };
        assert!(doc.validate().is_ok());
    }

    #[test]
    fn validate_sopkb_non_mapping_is_a_typed_error_not_a_crash() {
        // P-M26b: Python crashes with a raw AttributeError calling `.get("rule")` on a
        // truthy non-mapping `sopkb` value. Converted to a typed validation error.
        let doc = OkfDocument { frontmatter: fm(vec![("type", s("Doc")), ("sopkb", s("oops"))]), body: "x".into() };
        let err = doc.validate().unwrap_err().to_string();
        assert_eq!(err, "frontmatter.sopkb must be a mapping");
    }

    #[test]
    fn validate_rejects_missing_rule_obligation_action() {
        let rule = obj(vec![
            ("id", s("rule-1")),
            ("obligation", obj(vec![("fact", s("f")), ("label", s("l"))])), // missing "action"
        ]);
        let doc = OkfDocument {
            frontmatter: fm(vec![("type", s("SOP Decision Rule")), ("sopkb", obj(vec![("rule", rule)]))]),
            body: "x".into(),
        };
        let err = doc.validate().unwrap_err().to_string();
        assert!(err.contains("obligation.action"), "{err}");
    }

    #[test]
    fn validate_preserves_documented_gaps() {
        // condition.operator is never checked against {is_true,is_false}; otherwise
        // may be {} with no "label"; sources[].id/title are not required.
        let rule = obj(vec![
            ("id", s("r1")),
            ("obligation", obj(vec![("fact", s("f")), ("action", s("a")), ("label", s("l"))])),
            ("condition", obj(vec![("fact", s("f2")), ("operator", s("not-a-real-operator")), ("label", s("l2"))])),
            ("otherwise", obj(vec![])),
        ]);
        let doc = OkfDocument {
            frontmatter: fm(vec![
                ("type", s("SOP Decision Rule")),
                ("sources", OrderedJson::Array(vec![obj(vec![("resource", s("../x.md"))])])),
                ("sopkb", obj(vec![("rule", rule)])),
            ]),
            body: "x".into(),
        };
        assert!(doc.validate().is_ok());
    }

    #[test]
    fn validate_otherwise_action_required_must_be_boolean() {
        let rule = obj(vec![
            ("id", s("r1")),
            ("obligation", obj(vec![("fact", s("f")), ("action", s("a")), ("label", s("l"))])),
            ("otherwise", obj(vec![("action_required", s("yes"))])),
        ]);
        let doc = OkfDocument {
            frontmatter: fm(vec![("type", s("SOP Decision Rule")), ("sopkb", obj(vec![("rule", rule)]))]),
            body: "x".into(),
        };
        let err = doc.validate().unwrap_err().to_string();
        assert_eq!(err, "sopkb.rule.otherwise.action_required must be boolean");
    }

    // --- write_concept_doc / serialize -------------------------------------------

    #[test]
    fn write_concept_doc_persists_valid_rule_doc_pinned_path() {
        let dir = tempdir().unwrap();
        let frontmatter = fm(vec![
            ("type", s("SOP Decision Rule")),
            ("title", s("Check identity")),
            (
                "sources",
                OrderedJson::Array(vec![obj(vec![
                    ("id", s("src-1")),
                    ("resource", s("../sources/source.md")),
                    ("title", s("Source")),
                ])]),
            ),
            (
                "sopkb",
                obj(vec![(
                    "rule",
                    obj(vec![
                        ("id", s("rule-check-identity")),
                        (
                            "obligation",
                            obj(vec![
                                ("fact", s("patient_identity_confirmed")),
                                ("action", s("confirm")),
                                ("label", s("Patient identity confirmed")),
                            ]),
                        ),
                    ]),
                )]),
            ),
        ]);
        let result = write_concept_doc(dir.path(), "rules/check-identity", &frontmatter, "# Check identity\n", "sopkb/llm-author").unwrap();
        assert_eq!(result.path, "rules/check-identity.md");
        assert!(dir.path().join("rules").join("check-identity.md").exists());
    }

    #[test]
    fn write_concept_doc_rejects_invalid_rule_doc_missing_obligation_action() {
        let dir = tempdir().unwrap();
        let frontmatter = fm(vec![
            ("type", s("SOP Decision Rule")),
            (
                "sopkb",
                obj(vec![(
                    "rule",
                    obj(vec![
                        ("id", s("bad-rule")),
                        ("obligation", obj(vec![("fact", s("patient_identity_confirmed")), ("label", s("Patient identity confirmed"))])),
                    ]),
                )]),
            ),
        ]);
        let err = write_concept_doc(dir.path(), "rules/bad-rule", &frontmatter, "# Bad rule\n", "sopkb/llm-author").unwrap_err();
        assert!(err.to_string().contains("obligation.action"), "{err}");
        assert!(!dir.path().join("rules").join("bad-rule.md").exists(), "invalid doc must not be written");
    }

    #[test]
    fn write_concept_doc_bytes_is_utf8_byte_length_not_char_count() {
        // Python computes `"bytes": len(text.encode("utf-8"))` from the serialized
        // text BEFORE `Path.write_text()`'s own platform newline translation runs (LF
        // -> CRLF on Windows) -- so the returned count reflects the LF-only serialized
        // text, not necessarily the eventual on-disk file size. Verify against
        // `prepare_concept_doc`'s own `text`/`bytes` pair (the same source
        // `write_concept_doc` uses) rather than re-reading the file, to test the
        // documented semantics directly instead of an incidental platform detail.
        let frontmatter = fm(vec![("type", s("Note"))]);
        // "café" body: 4 chars, 5 UTF-8 bytes (é is 2 bytes) -- the rest of the
        // frontmatter/body wrapper is all ASCII, so only the "é" inflates byte count
        // beyond char count.
        let prepared = prepare_concept_doc("notes/x", &frontmatter, "café\n", "actor").unwrap();
        assert_eq!(prepared.bytes, prepared.text.len());
        assert!(prepared.bytes > prepared.text.chars().count(), "byte count must exceed char count for non-ASCII body");
    }

    #[test]
    fn write_concept_doc_defaults_generated_but_preserves_llm_supplied_value() {
        let dir = tempdir().unwrap();
        let frontmatter = fm(vec![("type", s("Note"))]);
        let result = write_concept_doc(dir.path(), "notes/a", &frontmatter, "x\n", "sopkb/llm-author").unwrap();
        let text = std::fs::read_to_string(dir.path().join("notes").join("a.md")).unwrap();
        assert!(text.contains("generated:"), "{text}");
        assert!(text.contains("actor: sopkb/llm-author"), "{text}");
        assert_eq!(result.path, "notes/a.md");

        // LLM-supplied "generated" (any shape) must be preserved untouched.
        let frontmatter2 = fm(vec![("type", s("Note")), ("generated", s("custom-value"))]);
        write_concept_doc(dir.path(), "notes/b", &frontmatter2, "x\n", "sopkb/llm-author").unwrap();
        let text2 = std::fs::read_to_string(dir.path().join("notes").join("b.md")).unwrap();
        assert!(text2.contains("generated: custom-value"), "{text2}");
    }

    #[test]
    fn serialize_layout_matches_python_exactly() {
        let frontmatter = fm(vec![("type", s("Note")), ("generated", obj(vec![("actor", s("a")), ("date", s("2026-01-01"))]))]);
        let doc = OkfDocument { frontmatter, body: "# Body\n".to_string() };
        let text = doc.serialize().unwrap();
        assert_eq!(text, "---\ntype: Note\ngenerated:\n  actor: a\n  date: '2026-01-01'\n---\n\n# Body\n");
    }

    #[test]
    fn serialize_adds_trailing_newline_to_body_when_missing() {
        let frontmatter = fm(vec![("type", s("Note"))]);
        let doc = OkfDocument { frontmatter, body: "no trailing newline".to_string() };
        let text = doc.serialize().unwrap();
        assert!(text.ends_with("no trailing newline\n"));
    }

    #[test]
    fn frontmatter_key_order_is_preserved_not_sorted() {
        // Deliberately non-alphabetical insertion order: sources, title, type.
        let frontmatter = fm(vec![
            ("sources", OrderedJson::Array(vec![])),
            ("title", s("Zeta first")),
            ("type", s("Note")),
            ("generated", obj(vec![("actor", s("a")), ("date", s("2026-01-01"))])),
        ]);
        let doc = OkfDocument { frontmatter, body: "x\n".to_string() };
        let text = doc.serialize().unwrap();
        let frontmatter_block = text.split("---").nth(1).unwrap();
        let order: Vec<&str> =
            frontmatter_block.lines().filter(|l| !l.starts_with(' ') && !l.trim().is_empty()).map(|l| l.split(':').next().unwrap()).collect();
        assert_eq!(order, vec!["sources", "title", "type", "generated"], "must preserve insertion order, not sort alphabetically");
    }
}
