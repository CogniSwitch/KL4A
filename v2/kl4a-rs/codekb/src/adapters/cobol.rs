//! Port of `kl4a/codekb/adapters/cobol.py`.
//!
//! The line-structured COBOL parser: fixed-format/free-format line
//! classification, division/section/paragraph/data/file symbol extraction,
//! statement-level fact extraction (COPY/CALL/PERFORM/OPEN/READ/WRITE), and
//! the relation-building pipeline (module `defines` symbol, plus
//! imports/calls/performs/reads/writes resolved against other modules and
//! symbols in the same parse batch).
//!
//! Ported symbol-for-symbol from the Python source (verified via tools-code
//! MCP `code_symbols_get` against every function/class/constant in the file).
//!
//! Dict-shaped records (`module`, `symbol`, `evidence`, `relation`) are kept
//! as `serde_json::Value` per the shared convention for `dict[str, Any]`
//! signatures — these are the same dynamic Code Knowledge Bundle record
//! shapes used by the rest of `codekb`, not stable types local to this file.
//! `CobolSource` / `CobolLine` (real Python dataclasses with a fixed 3-field
//! shape) and `Fact` (an internal, always-4-field record never seen outside
//! this pipeline) are ported as real structs instead.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use walkdir::WalkDir;

use crate::ids::{
    as_posix, code_evidence_id_for, code_module_id_for, code_relation_id_for,
    code_source_id_for, code_symbol_id_for,
};

/// `kl4a.codekb.adapters.cobol.COBOL_EXTENSIONS`.
pub const COBOL_EXTENSIONS: &[&str] = &[".cbl", ".cob", ".cpy"];

/// `kl4a.codekb.adapters.cobol._COBOL_KEYWORDS` — paragraph-name exclusion
/// list used only while scanning the PROCEDURE DIVISION (distinct from, and
/// smaller than, `cobol_adapter::COBOL_KEYWORDS`).
static PROCEDURE_KEYWORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "ACCEPT", "ADD", "CALL", "CLOSE", "COMPUTE", "COPY", "DISPLAY", "DIVIDE", "ELSE", "END",
        "EVALUATE", "EXIT", "GOBACK", "IF", "INITIALIZE", "MOVE", "MULTIPLY", "OPEN", "PERFORM",
        "READ", "SECTION", "STOP", "SUBTRACT", "UNTIL", "VARYING", "WHEN", "WRITE",
    ]
    .into_iter()
    .collect()
});

static PROGRAM_ID_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bPROGRAM-ID\.\s*([A-Z0-9_-]+)").unwrap());
static DIVISION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^\s*(IDENTIFICATION|ENVIRONMENT|DATA|PROCEDURE)\s+DIVISION\b").unwrap()
});
static SECTION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^\s*([A-Z0-9][A-Z0-9_-]*)\s+SECTION\.").unwrap());
static PARAGRAPH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^\s*([A-Z0-9][A-Z0-9_-]*)\.\s*$").unwrap());
static DATA_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^\s*(0[1-9]|[1-4][0-9]|66|77|78|88)\s+([A-Z0-9][A-Z0-9_-]*)\b").unwrap()
});
static FD_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^\s*FD\s+([A-Z0-9][A-Z0-9_-]*)\b").unwrap());
static COPY_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bCOPY\s+([A-Z0-9][A-Z0-9_-]*)\b").unwrap());
static CALL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\bCALL\s+['"]?([A-Z0-9][A-Z0-9_-]*)['"]?"#).unwrap());
static PERFORM_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bPERFORM\s+([A-Z0-9][A-Z0-9_-]*)\b").unwrap());
static OPEN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\bOPEN\s+(INPUT|OUTPUT|I-O|EXTEND)\s+([A-Z0-9][A-Z0-9_-]*)\b").unwrap()
});
static READ_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bREAD\s+([A-Z0-9][A-Z0-9_-]*)\b").unwrap());
static WRITE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bWRITE\s+([A-Z0-9][A-Z0-9_-]*)\b").unwrap());

/// Port of `kl4a.codekb.adapters.cobol.CobolSource` (a `@dataclass`).
#[derive(Debug, Clone)]
pub struct CobolSource {
    pub path: PathBuf,
    pub relative_path: String,
    pub text: String,
}

/// Port of `kl4a.codekb.adapters.cobol.CobolLine` (a `@dataclass`).
#[derive(Debug, Clone)]
pub struct CobolLine {
    pub number: usize,
    pub raw: String,
    pub code: String,
}

/// Port of the `CobolSource | tuple[str, str]` union accepted by
/// `normalize_source` / `parse_cobol_sources`.
pub enum CobolSourceInput {
    Source(CobolSource),
    /// `(relative_path, text)`.
    Tuple(String, String),
}

/// An internal statement-level fact (`imports`/`calls`/`performs`/`reads`/
/// `writes`) extracted from one logical line. Always exactly these 4 fields
/// in the Python source (a plain dict literal built the same way at every
/// call site), so it is ported as a real struct rather than `Value`.
#[derive(Debug, Clone)]
pub struct Fact {
    pub subject: String,
    pub predicate: String,
    pub target: String,
    pub line: usize,
}

/// The dict `parse_source` returns (`source`, `module`, `symbols`,
/// `evidence`, `parser_run`, `statements`) — a fixed shape reused verbatim by
/// `parse_cobol_sources`/`relation_records`, so ported as a real struct.
pub struct ParsedSource {
    pub source: CobolSource,
    pub module: Value,
    pub symbols: Vec<Value>,
    pub evidence: Vec<Value>,
    pub parser_run: Value,
    pub statements: Vec<Fact>,
}

/// Port of `kl4a.codekb.adapters.cobol.normalize_source`.
pub fn normalize_source(input: CobolSourceInput) -> CobolSource {
    match input {
        CobolSourceInput::Source(source) => source,
        CobolSourceInput::Tuple(relative_path, text) => CobolSource {
            path: PathBuf::from(&relative_path),
            relative_path: as_posix(&relative_path),
            text,
        },
    }
}

/// Port of `kl4a.codekb.adapters.cobol.parse_cobol_repository`.
///
/// "Parse supported COBOL files under a repository into code-KB-shaped
/// facts."
pub fn parse_cobol_repository(repo_dir: &Path) -> Result<Value> {
    let repo_dir = repo_dir
        .canonicalize()
        .unwrap_or_else(|_| repo_dir.to_path_buf());

    let mut matched: Vec<PathBuf> = WalkDir::new(&repo_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| COBOL_EXTENSIONS.contains(&format!(".{}", ext.to_lowercase()).as_str()))
                .unwrap_or(false)
        })
        .collect();
    matched.sort();

    let mut sources = Vec::with_capacity(matched.len());
    for path in matched {
        let relative = path.strip_prefix(&repo_dir).unwrap_or(path.as_path());
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        sources.push(CobolSourceInput::Source(CobolSource {
            path: path.clone(),
            relative_path: as_posix(relative),
            text,
        }));
    }
    parse_cobol_sources(sources)
}

/// Port of `kl4a.codekb.adapters.cobol.parse_cobol_sources`.
///
/// "Parse COBOL source text into modules, symbols, evidence, parser runs,
/// and relations."
pub fn parse_cobol_sources(sources: Vec<CobolSourceInput>) -> Result<Value> {
    let normalized: Vec<CobolSource> = sources.into_iter().map(normalize_source).collect();
    let parsed: Vec<ParsedSource> = normalized.iter().map(parse_source).collect();

    let modules: Vec<Value> = parsed.iter().map(|p| p.module.clone()).collect();
    let symbols: Vec<Value> = parsed.iter().flat_map(|p| p.symbols.clone()).collect();
    let evidence: Vec<Value> = parsed.iter().flat_map(|p| p.evidence.clone()).collect();
    let parser_runs: Vec<Value> = parsed.iter().map(|p| p.parser_run.clone()).collect();
    let relations = relation_records(&parsed, &modules, &symbols);
    let summary = relation_summary(&relations);

    Ok(json!({
        "modules": modules,
        "symbols": symbols,
        "evidence": evidence,
        "parser_runs": parser_runs,
        "relations": relations,
        "summary": summary,
    }))
}

/// Port of `kl4a.codekb.adapters.cobol.parse_source`.
pub fn parse_source(source: &CobolSource) -> ParsedSource {
    let lines = logical_lines(&source.text);
    let pid = program_id(&lines);
    let program_name = pid.clone().unwrap_or_else(|| {
        source
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_uppercase()
    });
    let module_name = format!("cobol.{program_name}");
    let mut module = module_record(source, &module_name);

    let parser_run = json!({
        "source_id": module["source_id"],
        "source_version_id": module["source_version_id"],
        "parser": "cobol-line-structured",
        "parser_version": "fixture-v1",
        "status": "parsed",
        "errors": Vec::<String>::new(),
    });

    let ext_is_cpy = source
        .path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("cpy"))
        .unwrap_or(false);
    let total_lines = source.text.lines().count().max(1);

    let root_kind = if ext_is_cpy && pid.is_none() { "copybook" } else { "program" };
    let root_signature = pid.as_ref().map(|_| format!("PROGRAM-ID. {program_name}"));

    let mut symbols: Vec<Value> = vec![symbol_record(
        source,
        &module,
        &program_name,
        root_kind,
        1,
        total_lines,
        "CobolCompilationUnit",
        root_signature.as_deref(),
    )];

    let mut current_division: Option<String> = None;
    let mut current_paragraph_idx: Option<usize> = None;
    let mut statements: Vec<Fact> = Vec::new();

    for line in &lines {
        if let Some(caps) = DIVISION_RE.captures(&line.code) {
            current_division = Some(caps[1].to_uppercase());
            current_paragraph_idx = None;
            continue;
        }

        if let Some(caps) = SECTION_RE.captures(&line.code) {
            if current_division.is_some() {
                let name = caps[1].to_uppercase();
                if name != "PROCEDURE" {
                    symbols.push(symbol_record(
                        source,
                        &module,
                        &name,
                        "section",
                        line.number,
                        line.number,
                        "CobolSection",
                        Some(line.code.trim_end()),
                    ));
                }
                continue;
            }
        }

        if let Some(caps) = DATA_RE.captures(&line.code) {
            let in_data_division = current_division.as_deref() == Some("DATA");
            if in_data_division || ext_is_cpy {
                symbols.push(symbol_record(
                    source,
                    &module,
                    &caps[2].to_uppercase(),
                    "data",
                    line.number,
                    line.number,
                    "CobolDataDescription",
                    Some(&compact_code(&line.code)),
                ));
            }
        }

        if let Some(caps) = FD_RE.captures(&line.code) {
            symbols.push(symbol_record(
                source,
                &module,
                &caps[1].to_uppercase(),
                "file",
                line.number,
                line.number,
                "CobolFileDescription",
                Some(&compact_code(&line.code)),
            ));
        }

        if let Some(caps) = PARAGRAPH_RE.captures(&line.code) {
            if current_division.as_deref() == Some("PROCEDURE") {
                let paragraph_name = caps[1].to_uppercase();
                if !PROCEDURE_KEYWORDS.contains(paragraph_name.as_str())
                    && !paragraph_name.starts_with("END-")
                {
                    if let Some(idx) = current_paragraph_idx {
                        let prev_start = symbols[idx]["line_start"].as_u64().unwrap_or(1) as usize;
                        symbols[idx]["line_end"] =
                            json!(prev_start.max(line.number.saturating_sub(1)));
                    }
                    symbols.push(symbol_record(
                        source,
                        &module,
                        &paragraph_name,
                        "paragraph",
                        line.number,
                        line.number,
                        "CobolParagraph",
                        Some(&format!("{paragraph_name}.")),
                    ));
                    current_paragraph_idx = Some(symbols.len() - 1);
                    continue;
                }
            }
        }

        let statement_subject = match current_paragraph_idx {
            Some(idx) => symbols[idx]["id"].as_str().unwrap_or_default().to_string(),
            None => symbols[0]["id"].as_str().unwrap_or_default().to_string(),
        };
        statements.extend(statement_facts(line, &statement_subject));
    }

    if let Some(idx) = current_paragraph_idx {
        let prev_start = symbols[idx]["line_start"].as_u64().unwrap_or(1) as usize;
        symbols[idx]["line_end"] = json!(prev_start.max(total_lines));
    }

    let imports: BTreeSet<String> = statements
        .iter()
        .filter(|fact| fact.predicate == "imports")
        .map(|fact| fact.target.clone())
        .collect();
    module["imports"] = json!(imports.into_iter().collect::<Vec<_>>());

    let evidence: Vec<Value> = symbols.iter().map(|s| evidence_record(source, s)).collect();
    for item in &evidence {
        let symbol_id = item["symbol_id"].as_str().unwrap_or_default();
        if let Some(sym) = symbols
            .iter_mut()
            .find(|s| s["id"].as_str() == Some(symbol_id))
        {
            sym["evidence_id"] = item["id"].clone();
        }
    }

    ParsedSource {
        source: source.clone(),
        module,
        symbols,
        evidence,
        parser_run,
        statements,
    }
}

/// Port of `kl4a.codekb.adapters.cobol.logical_lines`.
pub fn logical_lines(text: &str) -> Vec<CobolLine> {
    // UNCONFIRMED: Python's `str.splitlines()` also splits on a handful of
    // extra line-terminator code points (`\v`, `\f`, `\x1c`-`\x1e`, U+2028,
    // U+2029) that Rust's `str::lines()` does not. COBOL source containing
    // those separators is not something either side of this port has
    // evidence for, so this divergence is noted rather than special-cased.
    let mut lines = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let code = cobol_code_portion(raw);
        if !code.is_empty() {
            lines.push(CobolLine {
                number: index + 1,
                raw: raw.to_string(),
                code,
            });
        }
    }
    lines
}

/// Port of `kl4a.codekb.adapters.cobol.cobol_code_portion`.
pub fn cobol_code_portion(raw: &str) -> String {
    if raw.trim().is_empty() {
        return String::new();
    }
    let chars: Vec<char> = raw.chars().collect();
    if chars.len() > 6 {
        let prefix: String = chars[..6].iter().collect();
        let prefix_trimmed = prefix.trim();
        let is_digit_seq = !prefix_trimmed.is_empty()
            && prefix_trimmed.chars().all(|c| c.is_ascii_digit());
        if is_digit_seq || prefix_trimmed.is_empty() {
            let indicator = chars.get(6).copied();
            if indicator == Some('*') || indicator == Some('/') {
                return String::new();
            }
            let end = chars.len().min(72);
            let start = 7usize.min(end);
            return chars[start..end].iter().collect::<String>().trim().to_string();
        }
    }
    let stripped = raw.trim();
    if stripped.starts_with('*') {
        // Python: `stripped.startswith(("*", "*>"))` — every "*>"-prefixed
        // string is already caught by the plain "*" check.
        return String::new();
    }
    stripped.to_string()
}

/// Port of `kl4a.codekb.adapters.cobol.program_id`.
pub fn program_id(lines: &[CobolLine]) -> Option<String> {
    for line in lines {
        if let Some(caps) = PROGRAM_ID_RE.captures(&line.code) {
            return Some(caps[1].to_uppercase());
        }
    }
    None
}

/// Port of `kl4a.codekb.adapters.cobol.module_record`.
pub fn module_record(source: &CobolSource, module_name: &str) -> Value {
    let module_id = code_module_id_for(module_name);
    let source_id = code_source_id_for(&source.relative_path);
    json!({
        "id": module_id,
        "title": module_name.rsplit('.').next().unwrap_or(module_name),
        "qualified_name": module_name,
        "source_id": source_id,
        "source_version_id": format!("{source_id}:v1"),
        "file": source.relative_path,
        "language": "cobol",
        "imports": Vec::<String>::new(),
        "okf_path": format!("code/modules/{module_id}.md"),
    })
}

/// Port of `kl4a.codekb.adapters.cobol.symbol_record`.
#[allow(clippy::too_many_arguments)]
pub fn symbol_record(
    source: &CobolSource,
    module: &Value,
    title: &str,
    kind: &str,
    line_start: usize,
    line_end: usize,
    ast_node_type: &str,
    signature: Option<&str>,
) -> Value {
    let module_qualified_name = module["qualified_name"].as_str().unwrap_or_default();
    let module_title = module["title"].as_str().unwrap_or_default();
    let qualified_name = if matches!(kind, "program" | "copybook") && title == module_title {
        module_qualified_name.to_string()
    } else {
        format!("{module_qualified_name}.{title}")
    };
    let symbol_id = code_symbol_id_for(&qualified_name);
    json!({
        "id": symbol_id,
        "title": title,
        "kind": kind,
        "qualified_name": qualified_name,
        "module_id": module["id"],
        "module": module_qualified_name,
        "source_id": module["source_id"],
        "source_version_id": module["source_version_id"],
        "file": source.relative_path,
        "language": "cobol",
        "line_start": line_start,
        "line_end": line_end,
        "ast_node_type": ast_node_type,
        "signature": signature,
        "decorators": Vec::<String>::new(),
        "docstring": Value::Null,
        "okf_path": format!("code/symbols/{symbol_id}.md"),
        "evidence_id": code_evidence_id_for(&symbol_id),
    })
}

/// Port of `kl4a.codekb.adapters.cobol.statement_facts`.
pub fn statement_facts(line: &CobolLine, subject: &str) -> Vec<Fact> {
    let mut facts = Vec::new();
    for caps in COPY_RE.captures_iter(&line.code) {
        facts.push(Fact {
            subject: subject.to_string(),
            predicate: "imports".to_string(),
            target: caps[1].to_uppercase(),
            line: line.number,
        });
    }
    for caps in CALL_RE.captures_iter(&line.code) {
        facts.push(Fact {
            subject: subject.to_string(),
            predicate: "calls".to_string(),
            target: caps[1].to_uppercase(),
            line: line.number,
        });
    }
    for caps in PERFORM_RE.captures_iter(&line.code) {
        let target = caps[1].to_uppercase();
        if !matches!(target.as_str(), "UNTIL" | "VARYING" | "TIMES") {
            facts.push(Fact {
                subject: subject.to_string(),
                predicate: "performs".to_string(),
                target,
                line: line.number,
            });
        }
    }
    for caps in OPEN_RE.captures_iter(&line.code) {
        let mode = caps[1].to_uppercase();
        let predicate = if mode == "INPUT" { "reads" } else { "writes" };
        facts.push(Fact {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            target: caps[2].to_uppercase(),
            line: line.number,
        });
    }
    for caps in READ_RE.captures_iter(&line.code) {
        facts.push(Fact {
            subject: subject.to_string(),
            predicate: "reads".to_string(),
            target: caps[1].to_uppercase(),
            line: line.number,
        });
    }
    for caps in WRITE_RE.captures_iter(&line.code) {
        facts.push(Fact {
            subject: subject.to_string(),
            predicate: "writes".to_string(),
            target: caps[1].to_uppercase(),
            line: line.number,
        });
    }
    facts
}

/// Port of `kl4a.codekb.adapters.cobol.evidence_record`.
pub fn evidence_record(source: &CobolSource, symbol: &Value) -> Value {
    let lines: Vec<&str> = source.text.lines().collect();
    let start = (symbol["line_start"].as_u64().unwrap_or(1) as usize).max(1);
    let end = (symbol["line_end"].as_u64().unwrap_or(start as u64) as usize).max(start);
    let excerpt = if start >= 1 && start - 1 < lines.len() {
        lines[(start - 1)..end.min(lines.len())].join("\n")
    } else {
        String::new()
    };
    let evidence_id = code_evidence_id_for(symbol["id"].as_str().unwrap_or_default());
    json!({
        "id": evidence_id,
        "title": format!(
            "{} lines {}-{}",
            symbol["qualified_name"].as_str().unwrap_or_default(),
            start,
            end
        ),
        "source_id": symbol["source_id"],
        "source_version_id": symbol["source_version_id"],
        "symbol_id": symbol["id"],
        "file": source.relative_path,
        "language": "cobol",
        "line_start": start,
        "line_end": end,
        "ast_node_type": symbol["ast_node_type"],
        "span_status": "exact",
        "excerpt": excerpt,
        "okf_path": format!("evidence/{evidence_id}.md"),
    })
}

/// Port of `kl4a.codekb.adapters.cobol.relation_records`.
pub fn relation_records(
    parsed_sources: &[ParsedSource],
    modules: &[Value],
    symbols: &[Value],
) -> Vec<Value> {
    let mut relations = Vec::new();

    let modules_by_title: HashMap<String, &Value> = modules
        .iter()
        .map(|m| (m["title"].as_str().unwrap_or_default().to_uppercase(), m))
        .collect();

    let mut symbols_by_module_title: HashMap<(String, String), &Value> = HashMap::new();
    let mut symbols_by_title: HashMap<String, Vec<&Value>> = HashMap::new();
    for symbol in symbols {
        let module = symbol["module"].as_str().unwrap_or_default().to_string();
        let title_upper = symbol["title"].as_str().unwrap_or_default().to_uppercase();
        symbols_by_module_title.insert((module, title_upper.clone()), symbol);
        symbols_by_title.entry(title_upper).or_default().push(symbol);
    }

    for parsed in parsed_sources {
        let module = &parsed.module;
        let program_symbol = &parsed.symbols[0];
        for symbol in &parsed.symbols {
            relations.push(relation_record(
                module["id"].as_str().unwrap_or_default(),
                "defines",
                symbol["id"].as_str().unwrap_or_default(),
                "module",
                "symbol",
                module,
                "exact",
                1.0,
                "cobol-line-structured",
                Some(vec![symbol["evidence_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string()]),
            ));
        }

        for fact in &parsed.statements {
            let subject_symbol = parsed
                .symbols
                .iter()
                .find(|s| s["id"].as_str() == Some(fact.subject.as_str()))
                .unwrap_or(program_symbol);
            relations.push(relation_for_fact(
                fact,
                subject_symbol,
                module,
                &modules_by_title,
                &symbols_by_module_title,
                &symbols_by_title,
            ));
        }
    }

    dedupe_relations(relations)
}

/// Port of `kl4a.codekb.adapters.cobol.relation_for_fact`.
pub fn relation_for_fact(
    fact: &Fact,
    subject_symbol: &Value,
    module: &Value,
    modules_by_title: &HashMap<String, &Value>,
    symbols_by_module_title: &HashMap<(String, String), &Value>,
    symbols_by_title: &HashMap<String, Vec<&Value>>,
) -> Value {
    let target_name = fact.target.as_str();
    let predicate = fact.predicate.as_str();
    let module_qn = module["qualified_name"].as_str().unwrap_or_default().to_string();
    let target_symbol = symbols_by_module_title.get(&(module_qn, target_name.to_string()));
    let target_module = modules_by_title.get(target_name);

    if predicate == "imports" {
        if let Some(target_module) = target_module {
            return relation_record(
                module["id"].as_str().unwrap_or_default(),
                "imports",
                target_module["id"].as_str().unwrap_or_default(),
                "module",
                "module",
                module,
                "exact",
                0.96,
                "cobol-copybook-resolver",
                None,
            );
        }
    }

    let (object_value, object_kind, resolution_status, confidence): (String, &str, &str, f64) =
        if matches!(predicate, "performs" | "reads" | "writes") && target_symbol.is_some() {
            (
                target_symbol.unwrap()["id"].as_str().unwrap_or_default().to_string(),
                "symbol",
                "exact",
                0.94,
            )
        } else if predicate == "calls" && target_module.is_some() {
            (
                target_module.unwrap()["id"].as_str().unwrap_or_default().to_string(),
                "module",
                "exact",
                0.9,
            )
        } else if predicate == "calls"
            && symbols_by_title.get(target_name).map(|v| v.len()) == Some(1)
        {
            (
                symbols_by_title[target_name][0]["id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                "symbol",
                "exact",
                0.9,
            )
        } else {
            let object_value = if matches!(predicate, "calls" | "imports") {
                target_name.to_string()
            } else {
                format!("cobol-resource:{target_name}")
            };
            let object_kind = if predicate == "calls" {
                "external_program"
            } else if predicate == "imports" {
                "copybook"
            } else {
                "file_resource"
            };
            let resolution_status = if matches!(predicate, "calls" | "imports" | "performs") {
                "unresolved"
            } else {
                "inferred"
            };
            let confidence = if resolution_status == "unresolved" { 0.5 } else { 0.72 };
            (object_value, object_kind, resolution_status, confidence)
        };

    relation_record(
        &fact.subject,
        predicate,
        &object_value,
        "symbol",
        object_kind,
        subject_symbol,
        resolution_status,
        confidence,
        "cobol-line-structured",
        Some(vec![subject_symbol["evidence_id"]
            .as_str()
            .unwrap_or_default()
            .to_string()]),
    )
}

/// Port of `kl4a.codekb.adapters.cobol.relation_record`.
#[allow(clippy::too_many_arguments)]
pub fn relation_record(
    subject: &str,
    predicate: &str,
    object_value: &str,
    subject_kind: &str,
    object_kind: &str,
    source: &Value,
    resolution_status: &str,
    confidence: f64,
    resolver: &str,
    evidence_ids: Option<Vec<String>>,
) -> Value {
    let relation_id = code_relation_id_for(subject, predicate, object_value);
    json!({
        "id": relation_id,
        "type": "Code Knowledge Relation",
        "title": format!("{subject} {predicate} {object_value}"),
        "subject": subject,
        "predicate": predicate,
        "object": object_value,
        "subject_kind": subject_kind,
        "object_kind": object_kind,
        "relation": {
            "rdf_compatible": true,
            "resolution_status": resolution_status,
            "resolver": resolver,
            "confidence": round2(confidence),
        },
        "code": {
            "language": "cobol",
            "file": source.get("file").cloned().unwrap_or(Value::Null),
        },
        "evidence": evidence_ids.unwrap_or_default(),
        "okf_path": format!("relations/{relation_id}.md"),
    })
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Port of `kl4a.codekb.adapters.cobol.dedupe_relations`.
pub fn dedupe_relations(relations: Vec<Value>) -> Vec<Value> {
    let mut by_key: HashMap<(String, String, String), Value> = HashMap::new();
    for relation in relations {
        let key = (
            relation["subject"].as_str().unwrap_or_default().to_string(),
            relation["predicate"].as_str().unwrap_or_default().to_string(),
            relation["object"].as_str().unwrap_or_default().to_string(),
        );
        match by_key.get_mut(&key) {
            None => {
                by_key.insert(key, relation);
            }
            Some(existing) => {
                let mut evidence: BTreeSet<String> = existing["evidence"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                if let Some(arr) = relation["evidence"].as_array() {
                    for v in arr {
                        if let Some(s) = v.as_str() {
                            evidence.insert(s.to_string());
                        }
                    }
                }
                existing["evidence"] = json!(evidence.into_iter().collect::<Vec<_>>());

                let existing_status = existing["relation"]["resolution_status"]
                    .as_str()
                    .unwrap_or_default();
                let new_status = relation["relation"]["resolution_status"]
                    .as_str()
                    .unwrap_or_default();
                if existing_status != "exact" && new_status == "exact" {
                    *existing = relation;
                }
            }
        }
    }
    let mut out: Vec<Value> = by_key.into_values().collect();
    out.sort_by(|a, b| {
        let ak = (
            a["predicate"].as_str().unwrap_or_default(),
            a["subject"].as_str().unwrap_or_default(),
            a["object"].as_str().unwrap_or_default(),
        );
        let bk = (
            b["predicate"].as_str().unwrap_or_default(),
            b["subject"].as_str().unwrap_or_default(),
            b["object"].as_str().unwrap_or_default(),
        );
        ak.cmp(&bk)
    });
    out
}

/// Port of `kl4a.codekb.adapters.cobol.relation_summary`.
///
/// UNCONFIRMED / minor fidelity gap: Python's `by_predicate`/
/// `by_resolution_status` dicts preserve first-seen insertion order: this
/// workspace's `serde_json` is not confirmed to have the `preserve_order`
/// feature enabled, so `BTreeMap` (alphabetical key order) is used instead.
/// The *contents* (counts) are identical either way; only JSON key order
/// could differ from the Python output.
pub fn relation_summary(relations: &[Value]) -> Value {
    let mut by_predicate: std::collections::BTreeMap<String, u64> = Default::default();
    let mut by_status: std::collections::BTreeMap<String, u64> = Default::default();
    for relation in relations {
        let predicate = relation["predicate"].as_str().unwrap_or_default().to_string();
        *by_predicate.entry(predicate).or_insert(0) += 1;
        let status = relation["relation"]["resolution_status"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        *by_status.entry(status).or_insert(0) += 1;
    }
    json!({
        "count": relations.len(),
        "by_predicate": by_predicate,
        "by_resolution_status": by_status,
    })
}

/// Port of `kl4a.codekb.adapters.cobol.compact_code`.
pub fn compact_code(code: &str) -> String {
    code.split_whitespace().collect::<Vec<_>>().join(" ")
}
