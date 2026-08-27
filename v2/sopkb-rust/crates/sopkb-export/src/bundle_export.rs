//! `default_export_dir`/`export_artifact_path`/`export_path_for_manifest`,
//! `write_export_summary`, `export_bundle`. docs/port/port-mapping-c-review-export.md §3.1-3.2, §3.23.
//! `export_okf` is deliberately NOT ported (P-X24 [DROP]: unreferenced by CLI/web).

use crate::graph::export_graph_json;
use crate::rdf::export_rdf;
use crate::sync::sync_okf_bundle;
use sopkb_core::error::Result;
use sopkb_core::ids::slugify;
use sopkb_core::store;
use sopkb_fmt::{OrderedMap, YamlValue};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

/// `default_export_dir(bundle_dir) -> Path` (§3.2, P-X21 [PRESERVE bit-identically]):
/// branches on whether the parent directory is LITERALLY named `knowledge-bundles`
/// -- switching both the export LOCATION and the key used to name it (directory name
/// vs. manifest id). Requires a readable manifest; propagates its error.
pub fn default_export_dir(bundle_dir: &Path) -> Result<PathBuf> {
    let manifest = store::load_manifest(bundle_dir)?;
    let bundle_id = manifest
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| slugify(bundle_dir.file_name().and_then(|n| n.to_str()).unwrap_or("")));
    let parent = bundle_dir.parent().unwrap_or(bundle_dir);
    let parent_name = parent.file_name().and_then(|n| n.to_str());
    if parent_name == Some("knowledge-bundles") {
        let dir_name = bundle_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let grandparent = parent.parent().unwrap_or(parent);
        Ok(grandparent.join("exports").join(dir_name))
    } else {
        Ok(parent.join("exports").join(bundle_id))
    }
}

pub fn export_artifact_path(bundle_dir: &Path, relative_path: &str) -> Result<PathBuf> {
    Ok(default_export_dir(bundle_dir)?.join(relative_path))
}

/// Purely lexical native-path relative-path computation (no filesystem access, no
/// symlink resolution -- matching `os.path.relpath`'s own lexical `abspath`+`normpath`
/// semantics). Returns `None` on a Windows different-drive mismatch, mirroring
/// `os.path.relpath`'s `ValueError` in that case.
fn relpath_native(path: &Path, start: &Path) -> Option<String> {
    let path_comps: Vec<Component> = path.components().collect();
    let start_comps: Vec<Component> = start.components().collect();
    if let (Some(Component::Prefix(p1)), Some(Component::Prefix(p2))) = (path_comps.first(), start_comps.first()) {
        if p1.as_os_str() != p2.as_os_str() {
            return None;
        }
    }
    let common = path_comps.iter().zip(start_comps.iter()).take_while(|(a, b)| a == b).count();
    let mut rel_parts: Vec<String> = Vec::new();
    for _ in 0..(start_comps.len() - common) {
        rel_parts.push("..".to_string());
    }
    for comp in &path_comps[common..] {
        rel_parts.push(comp.as_os_str().to_string_lossy().to_string());
    }
    if rel_parts.is_empty() {
        Some(".".to_string())
    } else {
        Some(rel_parts.join("/"))
    }
}

/// `export_path_for_manifest(bundle_dir, path)` (§3.2): normally escapes the bundle
/// (`../exports/<id>/graph/graph.json`) -- P-X22 [PRESERVE]. Falls back to an
/// absolute POSIX-ized path when there is no relative form (different drives).
pub fn export_path_for_manifest(bundle_dir: &Path, path: &Path) -> String {
    match relpath_native(path, bundle_dir) {
        Some(rel) => rel,
        None => path.to_string_lossy().replace('\\', "/"),
    }
}

/// `write_export_summary` (§3.23, P-X23 [PRESERVE]): written ONLY by `export_bundle`
/// -- never by `sync_okf_bundle`, review actions, or `validate`. Lives OUTSIDE the
/// bundle, in the export directory, embedding OS-native path strings (backslashes on
/// Windows).
pub fn write_export_summary(bundle_dir: &Path, export_dir: &Path, exports: &[Value]) -> Result<()> {
    let mut lines = vec!["# Export Summary".to_string(), String::new(), format!("- Bundle: `{}`", bundle_dir.display()), format!("- Export directory: `{}`", export_dir.display()), String::new()];
    if exports.is_empty() {
        lines.push("- No exports generated.".to_string());
    } else {
        for entry in exports {
            lines.push(format!("- {}: `{}`", entry["type"].as_str().unwrap_or(""), entry["path"].as_str().unwrap_or("")));
        }
    }
    lines.push(String::new());
    std::fs::create_dir_all(export_dir)?;
    store::write_text_native(&export_dir.join("export_summary.md"), &lines.join("\n"))
}

fn export_entry_to_yaml(entry: &Value) -> YamlValue {
    let mut m = OrderedMap::new();
    m.insert("type", YamlValue::Scalar(entry["type"].as_str().unwrap_or("").to_string()));
    m.insert("path", YamlValue::Scalar(entry["path"].as_str().unwrap_or("").to_string()));
    YamlValue::Mapping(m)
}

/// `export_bundle(bundle_dir, formats) -> list[dict]` (§3.1). `sync_okf_bundle` runs
/// FIRST, always, regardless of `formats` -- an `"okf"` token in `formats` has no
/// effect of its own; the initial sync IS its effect.
///
/// P-X20 [PRESERVE]: the `manifest.exports` merge is order-sensitive -- surviving old
/// entries keep their original relative order (minus any `okf`/`okf_native` entries,
/// which are always dropped), then newly written types are appended/overwritten in
/// graph-json->rdf order. Exporting only one format leaves the other's stale manifest
/// entry untouched even though its file is not regenerated. `updated_at` is bumped
/// even when nothing in `exports` actually changed.
pub fn export_bundle(bundle_dir: &Path, formats: &[String]) -> Result<Vec<Value>> {
    sync_okf_bundle(bundle_dir)?;
    let export_dir = default_export_dir(bundle_dir)?;
    let mut exports: Vec<Value> = Vec::new();
    if formats.iter().any(|f| f == "graph-json") {
        exports.push(export_graph_json(bundle_dir, &export_dir)?);
    }
    if formats.iter().any(|f| f == "rdf") {
        exports.push(export_rdf(bundle_dir, &export_dir)?);
    }

    let mut manifest = store::load_manifest(bundle_dir)?;
    let mut existing: OrderedMap<YamlValue> = OrderedMap::new();
    if let Some(YamlValue::Sequence(entries)) = manifest.get("exports") {
        for entry in entries {
            if let YamlValue::Mapping(m) = entry {
                let etype = m.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if etype != "okf" && etype != "okf_native" {
                    existing.insert(etype, entry.clone());
                }
            }
        }
    }
    for entry in &exports {
        let etype = entry["type"].as_str().unwrap_or("").to_string();
        existing.insert(etype, export_entry_to_yaml(entry));
    }
    let new_exports_seq: Vec<YamlValue> = existing.iter().map(|(_, v)| v.clone()).collect();
    manifest.insert("exports", YamlValue::Sequence(new_exports_seq));
    manifest.insert("updated_at", YamlValue::Scalar(store::utc_now()));
    store::save_manifest_raw(bundle_dir, &manifest)?;

    write_export_summary(bundle_dir, &export_dir, &exports)?;
    Ok(exports)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relpath_native_worked_example() {
        let bundle = Path::new("C:/root/examples/glp1-healthcare/bundle");
        let target = Path::new("C:/root/examples/glp1-healthcare/exports/bundle/graph/graph.json");
        assert_eq!(relpath_native(target, bundle).unwrap(), "../exports/bundle/graph/graph.json");
    }

    #[test]
    fn default_export_dir_uses_dir_name_under_knowledge_bundles_else_manifest_id() {
        let dir = tempfile::tempdir().unwrap();
        let kb_root = dir.path().join("knowledge-bundles");
        let bundle_dir = kb_root.join("my-bundle-dir");
        sopkb_core::store::create_bundle(&bundle_dir, Some("Title")).unwrap();
        let export_dir = default_export_dir(&bundle_dir).unwrap();
        assert_eq!(export_dir, dir.path().join("exports").join("my-bundle-dir"));

        let plain_dir = dir.path().join("plain-parent").join("my-bundle-dir2");
        sopkb_core::store::create_bundle(&plain_dir, Some("Title")).unwrap();
        let export_dir2 = default_export_dir(&plain_dir).unwrap();
        // bundle_id is the slugified directory name (manifest id), not the dir name literally re-used here since they coincide
        assert_eq!(export_dir2, dir.path().join("plain-parent").join("exports").join("my-bundle-dir2"));
    }
}
