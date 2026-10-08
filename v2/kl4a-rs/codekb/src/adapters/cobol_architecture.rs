//! Port of `kl4a/codekb/adapters/cobol_architecture.py`.
//!
//! Derives the "COBOL slice" of a bundle's `code_architecture.json`: which
//! modules are programs vs. copybooks, fan-in for copybooks (who COPYs
//! them), file I/O (who reads/writes which data files), and unresolved
//! external CALL targets. Namespaced (`cobol_*` keys) so it can be merged
//! into the shared architecture document beside the Python-language views
//! without colliding.
//!
//! Ported symbol-for-symbol from the Python source (verified via tools-code
//! MCP `code_symbols_get` against every function/constant in the file).
//! Every dict-shaped input/output here (`source`, `module`, `symbol`,
//! `relation`, and this file's own output records) is the same dynamic Code
//! Knowledge Bundle record shape used across `codekb`, so all of it stays
//! `serde_json::Value` per the shared convention — this file defines no new
//! stable struct types, matching the Python (no dataclasses here either).

use std::collections::{BTreeSet, HashMap, HashSet};

use serde_json::{json, Value};

const PROGRAM_KINDS: &[&str] = &["program"];
const COPYBOOK_KINDS: &[&str] = &["copybook"];
const DATA_KINDS: &[&str] = &["data", "data_item", "record"];
const COPYBOOK_SUFFIXES: &[&str] = &[".cpy", ".copy"];

/// Port of `kl4a.codekb.adapters.cobol_architecture.detect_cobol_architecture`.
///
/// "Return the COBOL slice of a bundle's architecture state."
///
/// Takes owned `Value`s (expected to be JSON arrays), not `&[Value]`,
/// because that's the shape the confirmed cross-batch call site
/// (`architecture.rs::merge_adapter_architecture`) already builds and
/// passes: `inventory.get("sources").cloned().unwrap_or_else(|| json!([]))`,
/// `Value::Array(symbols.to_vec())`, etc. A non-array input (or an absent
/// key upstream defaulted to `json!([])`) degrades to an empty list rather
/// than panicking.
pub fn detect_cobol_architecture(sources: Value, modules: Value, symbols: Value, relations: Value) -> Value {
    let empty_arr: Vec<Value> = Vec::new();
    let sources = sources.as_array().cloned().unwrap_or_default();
    let modules = modules.as_array().cloned().unwrap_or(empty_arr.clone());
    let symbols = symbols.as_array().cloned().unwrap_or(empty_arr.clone());
    let relations = relations.as_array().cloned().unwrap_or(empty_arr);
    let sources: &[Value] = &sources;
    let modules: &[Value] = &modules;
    let symbols: &[Value] = &symbols;
    let relations: &[Value] = &relations;

    let cobol_files: HashSet<String> = sources
        .iter()
        .filter(|s| {
            s.get("language")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_lowercase()
                == "cobol"
        })
        // Python: `{s["path"] for s in sources if ...}` — a required key in
        // the Python source; `.and_then` here is a defensive relaxation for
        // malformed input rather than raising, since this port has no
        // KeyError equivalent to preserve.
        .filter_map(|s| s.get("path").and_then(Value::as_str).map(String::from))
        .collect();
    if cobol_files.is_empty() {
        return json!({});
    }

    let mods: Vec<Value> = modules
        .iter()
        .filter(|m| {
            m.get("file")
                .and_then(Value::as_str)
                .map(|f| cobol_files.contains(f))
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    let module_by_id: HashMap<String, Value> = mods
        .iter()
        .map(|m| (m["id"].as_str().unwrap_or_default().to_string(), m.clone()))
        .collect();

    let mut symbols_by_module: HashMap<String, Vec<Value>> = HashMap::new();
    for sym in symbols {
        if sym
            .get("file")
            .and_then(Value::as_str)
            .map(|f| cobol_files.contains(f))
            .unwrap_or(false)
        {
            let module_id = sym.get("module_id").and_then(Value::as_str).unwrap_or_default();
            symbols_by_module
                .entry(module_id.to_string())
                .or_default()
                .push(sym.clone());
        }
    }

    let symbol_by_id: HashMap<String, Value> = symbols
        .iter()
        .map(|s| (s["id"].as_str().unwrap_or_default().to_string(), s.clone()))
        .collect();

    let programs_out = programs(&mods, &symbols_by_module, relations, &module_by_id, &symbol_by_id);
    let copybooks_out = copybooks(&mods, &symbols_by_module, relations, &module_by_id, &symbol_by_id);
    let file_io_out = file_io(symbols, relations, &cobol_files);
    let external_calls_out = external_calls(relations, &module_by_id, &symbol_by_id);

    json!({
        "cobol_programs": programs_out.clone(),
        "cobol_copybooks": copybooks_out.clone(),
        "cobol_file_io": file_io_out.clone(),
        "cobol_external_calls": external_calls_out.clone(),
        "summary": {
            "cobol_programs": programs_out.len(),
            "cobol_copybooks": copybooks_out.len(),
            "cobol_file_io": file_io_out.len(),
            "cobol_external_calls": external_calls_out.len(),
        },
    })
}

/// Port of `kl4a.codekb.adapters.cobol_architecture._kind_of`.
///
/// "program or copybook. The parser is meant to tag a `.cpy` compilation
/// unit as a copybook but currently tags it `program`, so the file
/// extension is the reliable signal and the symbol kind is only trusted
/// when it actually says copybook."
fn kind_of(module: &Value, symbols_by_module: &HashMap<String, Vec<Value>>) -> &'static str {
    let module_id = module["id"].as_str().unwrap_or_default();
    if let Some(syms) = symbols_by_module.get(module_id) {
        for sym in syms {
            if let Some(kind) = sym.get("kind").and_then(Value::as_str) {
                if COPYBOOK_KINDS.contains(&kind) {
                    return "copybook";
                }
            }
        }
    }
    let file = module
        .get("file")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    if COPYBOOK_SUFFIXES.iter().any(|suffix| file.ends_with(suffix)) {
        return "copybook";
    }
    "program"
}

/// Port of `kl4a.codekb.adapters.cobol_architecture._subject_module`.
///
/// "The module a relation originates from, whichever end it was recorded
/// on."
fn subject_module(
    rel: &Value,
    module_by_id: &HashMap<String, Value>,
    symbol_module: &HashMap<String, String>,
) -> String {
    let subject = rel.get("subject").and_then(Value::as_str).unwrap_or_default();
    if module_by_id.contains_key(subject) {
        subject.to_string()
    } else {
        symbol_module.get(subject).cloned().unwrap_or_default()
    }
}

/// Port of `kl4a.codekb.adapters.cobol_architecture._plain`.
///
/// "A readable name for a relation target, which may be an id of either
/// kind."
fn plain(value: &str, module_by_id: &HashMap<String, Value>, symbol_by_id: &HashMap<String, Value>) -> String {
    if let Some(module) = module_by_id.get(value) {
        return name_of(Some(module));
    }
    if let Some(symbol) = symbol_by_id.get(value) {
        return symbol
            .get("title")
            .and_then(Value::as_str)
            .or_else(|| symbol.get("qualified_name").and_then(Value::as_str))
            .unwrap_or(value)
            .to_string();
    }
    value.to_string()
}

/// Port of `kl4a.codekb.adapters.cobol_architecture._name_of`.
fn name_of(module: Option<&Value>) -> String {
    match module {
        None => String::new(),
        Some(m) => m
            .get("qualified_name")
            .and_then(Value::as_str)
            .or_else(|| m.get("title").and_then(Value::as_str))
            .or_else(|| m.get("id").and_then(Value::as_str))
            .unwrap_or("")
            .to_string(),
    }
}

/// Port of `kl4a.codekb.adapters.cobol_architecture._programs`.
fn programs(
    mods: &[Value],
    symbols_by_module: &HashMap<String, Vec<Value>>,
    relations: &[Value],
    module_by_id: &HashMap<String, Value>,
    symbol_by_id: &HashMap<String, Value>,
) -> Vec<Value> {
    let mut symbol_module: HashMap<String, String> = HashMap::new();
    for (module_id, syms) in symbols_by_module {
        for sym in syms {
            symbol_module.insert(sym["id"].as_str().unwrap_or_default().to_string(), module_id.clone());
        }
    }

    let mut copies: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut calls: HashMap<String, BTreeSet<String>> = HashMap::new();
    for rel in relations {
        let origin = subject_module(rel, module_by_id, &symbol_module);
        if origin.is_empty() {
            continue;
        }
        let target = rel.get("object").and_then(Value::as_str).unwrap_or_default();
        let predicate = rel.get("predicate").and_then(Value::as_str).unwrap_or_default();
        if predicate == "imports" {
            let name = plain(target, module_by_id, symbol_by_id);
            if !name.is_empty() {
                copies.entry(origin).or_default().insert(name);
            }
        } else if predicate == "calls" {
            let name = plain(target, module_by_id, symbol_by_id);
            if !name.is_empty() {
                calls.entry(origin).or_default().insert(name);
            }
        }
    }

    let mut out = Vec::new();
    for module in mods {
        if kind_of(module, symbols_by_module) != PROGRAM_KINDS[0] {
            continue;
        }
        let module_id = module["id"].as_str().unwrap_or_default().to_string();
        let empty: Vec<Value> = Vec::new();
        let syms = symbols_by_module.get(&module_id).unwrap_or(&empty);
        let name = module
            .get("qualified_name")
            .and_then(Value::as_str)
            .or_else(|| module.get("title").and_then(Value::as_str))
            .unwrap_or(&module_id)
            .to_string();
        let paragraphs = syms
            .iter()
            .filter(|s| s.get("kind").and_then(Value::as_str) == Some("paragraph"))
            .count();
        let sections = syms
            .iter()
            .filter(|s| s.get("kind").and_then(Value::as_str) == Some("section"))
            .count();
        let data_items = syms
            .iter()
            .filter(|s| DATA_KINDS.contains(&s.get("kind").and_then(Value::as_str).unwrap_or_default()))
            .count();
        out.push(json!({
            "id": module_id,
            "name": name,
            "file": module.get("file").and_then(Value::as_str).unwrap_or_default(),
            "paragraphs": paragraphs,
            "sections": sections,
            "data_items": data_items,
            "copybooks": copies.get(&module_id).map(|s| s.iter().filter(|c| !c.is_empty()).cloned().collect::<Vec<_>>()).unwrap_or_default(),
            "calls": calls.get(&module_id).map(|s| s.iter().filter(|c| !c.is_empty()).cloned().collect::<Vec<_>>()).unwrap_or_default(),
        }));
    }
    out.sort_by(|a, b| a["name"].as_str().unwrap_or_default().cmp(b["name"].as_str().unwrap_or_default()));
    out
}

/// Port of `kl4a.codekb.adapters.cobol_architecture._copybooks`.
///
/// "Copybooks in the repo, and the programs that COPY them. Fan-in is the
/// useful direction: a copybook edited without checking who copies it is
/// the classic way to break a COBOL build."
fn copybooks(
    mods: &[Value],
    symbols_by_module: &HashMap<String, Vec<Value>>,
    relations: &[Value],
    module_by_id: &HashMap<String, Value>,
    symbol_by_id: &HashMap<String, Value>,
) -> Vec<Value> {
    let mut used_by: HashMap<String, BTreeSet<String>> = HashMap::new();
    for rel in relations {
        if rel.get("predicate").and_then(Value::as_str) != Some("imports") {
            continue;
        }
        let subject = rel.get("subject").and_then(Value::as_str).unwrap_or_default();
        let origin = module_by_id.get(subject);
        let target = plain(
            rel.get("object").and_then(Value::as_str).unwrap_or_default(),
            module_by_id,
            symbol_by_id,
        );
        if !target.is_empty() {
            let user = match origin {
                Some(m) => name_of(Some(m)),
                None => subject.to_string(),
            };
            used_by.entry(target.to_uppercase()).or_default().insert(user);
        }
    }

    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let empty: Vec<Value> = Vec::new();
    for module in mods {
        if kind_of(module, symbols_by_module) != COPYBOOK_KINDS[0] {
            continue;
        }
        let name = name_of(Some(module));
        seen.insert(name.to_uppercase());
        let module_id = module["id"].as_str().unwrap_or_default().to_string();
        let fields = symbols_by_module
            .get(&module_id)
            .unwrap_or(&empty)
            .iter()
            .filter(|s| DATA_KINDS.contains(&s.get("kind").and_then(Value::as_str).unwrap_or_default()))
            .count();
        out.push(json!({
            "id": module_id,
            "name": name.clone(),
            "file": module.get("file").and_then(Value::as_str).unwrap_or_default(),
            "fields": fields,
            "in_repo": true,
            "used_by": used_by.get(&name.to_uppercase()).map(|s| s.iter().cloned().collect::<Vec<_>>()).unwrap_or_default(),
        }));
    }
    // "A COPY of something the repo does not contain is worth showing, not
    // hiding."
    for (name, users) in &used_by {
        if !seen.contains(name) {
            out.push(json!({
                "id": format!("copybook:{name}"),
                "name": name,
                "file": "",
                "fields": 0,
                "in_repo": false,
                "used_by": users.iter().cloned().collect::<Vec<_>>(),
            }));
        }
    }
    out.sort_by(|a, b| {
        let a_key = (!a["in_repo"].as_bool().unwrap_or(false), a["name"].as_str().unwrap_or_default().to_string());
        let b_key = (!b["in_repo"].as_bool().unwrap_or(false), b["name"].as_str().unwrap_or_default().to_string());
        a_key.cmp(&b_key)
    });
    out
}

/// Port of `kl4a.codekb.adapters.cobol_architecture._file_io`.
///
/// "Data files each program reads and writes."
fn file_io(symbols: &[Value], relations: &[Value], cobol_files: &HashSet<String>) -> Vec<Value> {
    let symbol_by_id: HashMap<String, &Value> = symbols
        .iter()
        .map(|s| (s["id"].as_str().unwrap_or_default().to_string(), s))
        .collect();
    let mut readers: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut writers: HashMap<String, BTreeSet<String>> = HashMap::new();
    for rel in relations {
        let predicate = rel.get("predicate").and_then(Value::as_str).unwrap_or_default();
        if predicate != "reads" && predicate != "writes" {
            continue;
        }
        let subject_id = rel.get("subject").and_then(Value::as_str).unwrap_or_default();
        let subject = symbol_by_id.get(subject_id).copied();
        if let Some(s) = subject {
            let file = s.get("file").and_then(Value::as_str).unwrap_or_default();
            if !cobol_files.contains(file) {
                continue;
            }
        }
        let target = rel.get("object").and_then(Value::as_str).unwrap_or_default();
        let name = target.rsplit("cobol-resource:").next().unwrap_or(target);
        let who = match subject {
            Some(s) => s.get("qualified_name").and_then(Value::as_str).unwrap_or("").to_string(),
            None => subject_id.to_string(),
        };
        let map = if predicate == "reads" { &mut readers } else { &mut writers };
        map.entry(name.to_string()).or_default().insert(who);
    }

    let mut names: BTreeSet<String> = readers.keys().cloned().collect();
    names.extend(writers.keys().cloned());
    names
        .into_iter()
        .filter(|n| !n.is_empty())
        .map(|name| {
            json!({
                "name": name.clone(),
                "reads": readers.get(&name).map(|s| s.iter().cloned().collect::<Vec<_>>()).unwrap_or_default(),
                "writes": writers.get(&name).map(|s| s.iter().cloned().collect::<Vec<_>>()).unwrap_or_default(),
            })
        })
        .collect()
}

/// Port of `kl4a.codekb.adapters.cobol_architecture._external_calls`.
///
/// "CALL targets that are not programs in this repository."
fn external_calls(
    relations: &[Value],
    module_by_id: &HashMap<String, Value>,
    symbol_by_id: &HashMap<String, Value>,
) -> Vec<Value> {
    let mut called_by: HashMap<String, BTreeSet<String>> = HashMap::new();
    for rel in relations {
        if rel.get("predicate").and_then(Value::as_str) != Some("calls") {
            continue;
        }
        let target = rel.get("object").and_then(Value::as_str).unwrap_or_default();
        // "resolved inside the repo; the relations view already has it"
        if module_by_id.contains_key(target) || symbol_by_id.contains_key(target) {
            continue;
        }
        let subject_id = rel.get("subject").and_then(Value::as_str).unwrap_or_default();
        let origin = module_by_id.get(subject_id).or_else(|| symbol_by_id.get(subject_id));
        let name = match origin {
            Some(m) => name_of(Some(m)),
            None => subject_id.to_string(),
        };
        called_by.entry(target.to_string()).or_default().insert(name);
    }
    let mut out: Vec<Value> = called_by
        .into_iter()
        .filter(|(name, _)| !name.is_empty())
        .map(|(name, users)| {
            json!({ "name": name, "called_by": users.into_iter().collect::<Vec<_>>() })
        })
        .collect();
    out.sort_by(|a, b| a["name"].as_str().unwrap_or_default().cmp(b["name"].as_str().unwrap_or_default()));
    out
}
