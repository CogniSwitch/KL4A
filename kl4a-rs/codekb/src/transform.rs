//! Port of `kl4a/codekb/transform.py`.
//!
//! Reads a canonicalized code-knowledge bundle and produces a "transform
//! plan" plus, from that plan, a generated target-language (currently only
//! `target = "python"`) scaffold repository: dataclass models for preserved
//! state contracts, a workflow module with one stub function per preserved
//! operation, a hand-written payroll reference implementation, generated
//! pytest files (a smoke test and, when sample I/O exists, a behavior
//! equivalence test), a README, and a `pyproject.toml`.
//!
//! Every function below has a 1:1 Python counterpart, ported from the exact
//! source excerpt returned by the tools-code MCP server's `code_symbols_get`
//! (line ranges cited per function). All 13 symbols in `transform.py` are
//! covered — see the handback ledger.
//!
//! ## Cross-batch dependencies (NOT part of this batch — report only)
//!
//! This batch was scoped to `kl4a/codekb/transform.py` only. Three sibling
//! modules it calls into are owned by other batches and are expected here
//! as:
//!
//! - `crate::ids::slugify(name: &str) -> String` — port of
//!   `kl4a.kl4a.ids.slugify`: lower-cases, replaces runs of non-alphanumerics
//!   with `-`, and strips leading/trailing `-` (falls back to `"item"` if
//!   the result would be empty).
//! - `crate::canonical::canonical_name(name: &str) -> String` — port of
//!   `kl4a.codekb.canonical.canonical_name`: splits a `snake_case` or
//!   `CamelCase` identifier into lowercase words joined by single spaces.
//! - `crate::canonical::canonicalize_code_bundle(bundle_dir: &Path) ->
//!   anyhow::Result<serde_json::Value>` — port of
//!   `kl4a.codekb.canonical.canonicalize_code_bundle`. Fallible because it
//!   writes `code_canonical.json` via `state::write_json` internally.
//! - `crate::state::read_json(path: &Path, default: serde_json::Value) ->
//!   serde_json::Value` — port of `kl4a.codekb.state.read_json`. **Infallible
//!   by design**: the Python source (retrieved via MCP, `kl4a/codekb/state.py`
//!   lines 76-94) explicitly swallows a missing file, an unreadable file, an
//!   empty/whitespace-only file, and a JSON parse error, returning `default`
//!   in every case rather than raising — this is documented in its own
//!   docstring ("Tolerate a torn or empty file rather than raising"). The
//!   Rust port should not return `Result` for this reason; do not add `?`
//!   handling that would make a missing/corrupt plan file panic or bubble an
//!   error where Python would have quietly used the default.
//! - `crate::state::write_json(path: &Path, data: &serde_json::Value) ->
//!   anyhow::Result<()>` — port of `kl4a.codekb.state.write_json`. Fallible:
//!   the Python source calls an atomic-write helper with no try/except, so
//!   an OS-level write failure propagates as an uncaught exception.
//!
//! This file will not compile until those three modules land with matching
//! signatures; the coordinator should wire the real paths in during lib.rs
//! assembly (see the handback report for the exact `mod`/`use` lines
//! recommended).
//!
//! ## Other notes
//!
//! - `create_transform_plan`'s Python signature takes `target` as a
//!   keyword-only argument (`*, target: str, out_dir: Path`); Rust has no
//!   direct equivalent, so it is just a positional parameter here.
//! - The Python `FileNotFoundError` raised by `generate_target_repo` for a
//!   missing/invalid plan is surfaced here as a plain `anyhow` error rather
//!   than a distinct type. If a later batch's CLI layer needs to
//!   specifically detect "plan missing" (as opposed to any other failure) to
//!   choose an exit code or message, that will need a `thiserror` variant
//!   instead of matching on this error's message text.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

// UNCONFIRMED wiring — see module doc comment "Cross-batch dependencies".
use crate::canonical::{canonical_name, canonicalize_code_bundle};
use crate::ids::slugify;
use crate::state::{read_json, write_json};

// ---------------------------------------------------------------------
// kl4a.codekb.transform.python_identifier (lines 139-145)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.python_identifier`.
///
/// Turns an arbitrary name into a valid Python (snake_case-ish) identifier:
/// slugify, then swap `-` for `_`; empty result falls back to
/// `"migrated_operation"`; a leading digit gets an `op_` prefix.
pub fn python_identifier(name: &str) -> String {
    let mut candidate = slugify(name).replace('-', "_");
    if candidate.is_empty() {
        return "migrated_operation".to_string();
    }
    if candidate.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        candidate = format!("op_{candidate}");
    }
    candidate
}

// ---------------------------------------------------------------------
// kl4a.codekb.transform.python_class_name (lines 148-159)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.python_class_name`.
///
/// Slugifies the name, splits on `-`/`_`, upper-cases just the first
/// character of each word (Python's `word[:1].upper() + word[1:]`, i.e. the
/// rest of the word is left as-is, not lower-cased) and concatenates
/// (PascalCase). Empty result falls back to `"MigratedRecord"`; a leading
/// digit gets a `Record` prefix.
pub fn python_class_name(name: &str) -> String {
    let normalized = slugify(name).replace('-', " ").replace('_', " ");
    let mut candidate = String::new();
    for word in normalized.split_whitespace() {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            candidate.extend(first.to_uppercase());
            candidate.push_str(chars.as_str());
        }
    }
    if candidate.is_empty() {
        return "MigratedRecord".to_string();
    }
    if candidate.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        candidate = format!("Record{candidate}");
    }
    candidate
}

// ---------------------------------------------------------------------
// kl4a.codekb.transform.behavior_contracts (lines 117-136)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.behavior_contracts`.
///
/// Looks for `<repository.path>/testdata/employees.in` and
/// `.../expected_payroll.out`; if the canonical bundle has no repository
/// path, or either file is missing, returns an empty list (no behavior
/// contract). Otherwise returns a single `sample_io` contract carrying both
/// files' contents split into lines.
pub fn behavior_contracts(canonical: &Value) -> Result<Vec<Value>> {
    let repo_path = canonical
        .get("repository")
        .and_then(|r| r.get("path"))
        .and_then(|p| p.as_str())
        .filter(|s| !s.is_empty());
    let repo_path = match repo_path {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };

    let testdata_dir = Path::new(repo_path).join("testdata");
    let employees = testdata_dir.join("employees.in");
    let expected = testdata_dir.join("expected_payroll.out");
    if !employees.exists() || !expected.exists() {
        return Ok(Vec::new());
    }

    let employees_text = fs::read_to_string(&employees)
        .with_context(|| format!("failed to read {}", employees.display()))?;
    let expected_text = fs::read_to_string(&expected)
        .with_context(|| format!("failed to read {}", expected.display()))?;

    let input_lines: Vec<Value> = employees_text
        .lines()
        .map(|l| Value::String(l.to_string()))
        .collect();
    let expected_lines: Vec<Value> = expected_text
        .lines()
        .map(|l| Value::String(l.to_string()))
        .collect();

    let input_name = employees
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("employees.in");
    let expected_name = expected
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("expected_payroll.out");

    Ok(vec![json!({
        "id": "behavior-contract-payroll-output",
        "name": "payroll output equivalence",
        "kind": "sample_io",
        "input_name": input_name,
        "expected_name": expected_name,
        "input_lines": input_lines,
        "expected_lines": expected_lines,
    })])
}

// ---------------------------------------------------------------------
// kl4a.codekb.transform.create_transform_plan (lines 12-57)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.create_transform_plan`.
///
/// Only `target == "python"` is supported (matches the Python source's own
/// restriction, enforced with an early error). Canonicalizes the source
/// bundle, keeps only `program`/`paragraph` artifacts as "operations",
/// carries every state contract forward (enriched with a generated Python
/// class name and `disposition: "preserved"`), and derives behavior
/// contracts from on-disk sample I/O. Writes `transform_plan.json` and a
/// rendered `README.md` into `out_dir`.
pub fn create_transform_plan(source_bundle_dir: &Path, target: &str, out_dir: &Path) -> Result<Value> {
    if target != "python" {
        bail!("initial transform planner supports target=python");
    }

    let canonical = canonicalize_code_bundle(source_bundle_dir)?;

    let operations_in: Vec<Value> = canonical
        .get("artifacts")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|artifact| {
            matches!(
                artifact.get("kind").and_then(|k| k.as_str()),
                Some("program") | Some("paragraph")
            )
        })
        .collect();

    let mut plan_operations = Vec::with_capacity(operations_in.len());
    for artifact in &operations_in {
        let id = artifact
            .get("id")
            .and_then(|v| v.as_str())
            .context("artifact missing id")?;
        let qualified_name = artifact
            .get("qualified_name")
            .and_then(|v| v.as_str())
            .context("artifact missing qualified_name")?;
        let name = artifact
            .get("name")
            .and_then(|v| v.as_str())
            .context("artifact missing name")?;
        // Python: python_identifier(canonical_name(artifact["name"]) or artifact["name"])
        let canonical_form = canonical_name(name);
        let source_for_ident = if canonical_form.is_empty() { name } else { &canonical_form };
        let target_name = python_identifier(source_for_ident);
        plan_operations.push(json!({
            "source_artifact_id": id,
            "source_qualified_name": qualified_name,
            "target_name": target_name,
            "target_kind": "function",
            "disposition": "preserved",
        }));
    }

    let state_contracts_in: Vec<Value> = canonical
        .get("state_contracts")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut state_contracts = Vec::with_capacity(state_contracts_in.len());
    for contract in state_contracts_in {
        let mut contract_map = contract
            .as_object()
            .cloned()
            .context("state contract entry is not an object")?;
        let symbol_id = contract_map
            .get("symbol_id")
            .and_then(|v| v.as_str())
            .context("state contract missing symbol_id")?
            .to_string();
        let name = contract_map
            .get("name")
            .and_then(|v| v.as_str())
            .context("state contract missing name")?
            .to_string();
        // Python: python_class_name(canonical_name(contract["name"]) or contract["name"])
        let canonical_form = canonical_name(&name);
        let source_for_class = if canonical_form.is_empty() { name.as_str() } else { canonical_form.as_str() };
        let target_name = python_class_name(source_for_class);

        contract_map.insert("source_artifact_id".to_string(), Value::String(symbol_id));
        contract_map.insert("target_name".to_string(), Value::String(target_name));
        contract_map.insert("target_kind".to_string(), Value::String("class".to_string()));
        contract_map.insert("disposition".to_string(), Value::String("preserved".to_string()));
        state_contracts.push(Value::Object(contract_map));
    }

    let source_bundle_name = source_bundle_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();

    let mut plan = Map::new();
    plan.insert("source_bundle".to_string(), Value::String(source_bundle_name));
    plan.insert("target".to_string(), Value::String(target.to_string()));
    plan.insert("package".to_string(), Value::String("migrated".to_string()));
    plan.insert(
        "behavior_contracts".to_string(),
        Value::Array(behavior_contracts(&canonical)?),
    );
    plan.insert("operations".to_string(), Value::Array(plan_operations));
    plan.insert("state_contracts".to_string(), Value::Array(state_contracts));
    let plan = Value::Object(plan);

    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create out_dir {}", out_dir.display()))?;
    write_json(&out_dir.join("transform_plan.json"), &plan)?;
    fs::write(out_dir.join("README.md"), render_transform_plan(&plan)?)
        .context("failed to write transform plan README.md")?;

    Ok(plan)
}

// ---------------------------------------------------------------------
// kl4a.codekb.transform.generate_target_repo (lines 60-114)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.generate_target_repo`.
///
/// Reads `transform_plan.json` from `plan_dir` (via the tolerant
/// `state::read_json`, so a missing/corrupt file reads back as
/// `serde_json::Value::Null`, not an error) and, if it isn't a JSON object,
/// raises (mirrors Python's `FileNotFoundError`). Otherwise writes a
/// `src/<package>/{__init__,models,payroll,workflow}.py`,
/// `tests/test_workflow.py`, optionally `tests/test_behavior_equivalence.py`
/// (only when `plan.behavior_contracts` is a non-empty truthy value),
/// `README.md`, and `pyproject.toml` under `out_repo_dir`.
///
/// Note (confirmed from source, not a guess): the returned/`generated_target.json`
/// `files` list is a fixed 7-entry list that does **not** include
/// `tests/test_behavior_equivalence.py` even when that file is written. This
/// is a genuine quirk of the original Python — preserved here deliberately,
/// not a bug in the port.
pub fn generate_target_repo(plan_dir: &Path, out_repo_dir: &Path) -> Result<Value> {
    let plan_path = plan_dir.join("transform_plan.json");
    let plan = read_json(&plan_path, Value::Null);
    if !plan.is_object() {
        bail!("missing transform plan: {}", plan_path.display());
    }

    // Python: plan.get("package") or "migrated" -- falls back on None AND on "".
    let package = plan
        .get("package")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("migrated")
        .to_string();

    let package_dir = out_repo_dir.join("src").join(&package);
    let tests_dir = out_repo_dir.join("tests");
    fs::create_dir_all(&package_dir)
        .with_context(|| format!("failed to create {}", package_dir.display()))?;
    fs::create_dir_all(&tests_dir)
        .with_context(|| format!("failed to create {}", tests_dir.display()))?;

    fs::write(package_dir.join("__init__.py"), "").context("failed to write __init__.py")?;
    fs::write(package_dir.join("models.py"), render_python_models(&plan)?)
        .context("failed to write models.py")?;
    fs::write(package_dir.join("payroll.py"), render_python_payroll(&plan))
        .context("failed to write payroll.py")?;
    fs::write(package_dir.join("workflow.py"), render_python_workflow(&plan)?)
        .context("failed to write workflow.py")?;
    fs::write(tests_dir.join("test_workflow.py"), render_python_tests(&plan)?)
        .context("failed to write test_workflow.py")?;

    let has_behavior_contracts = plan
        .get("behavior_contracts")
        .map(|v| match v {
            Value::Array(a) => !a.is_empty(),
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::String(s) => !s.is_empty(),
            Value::Object(o) => !o.is_empty(),
            Value::Number(n) => n.as_f64().map_or(true, |f| f != 0.0),
        })
        .unwrap_or(false);
    if has_behavior_contracts {
        fs::write(
            tests_dir.join("test_behavior_equivalence.py"),
            render_python_behavior_tests(&plan)?,
        )
        .context("failed to write test_behavior_equivalence.py")?;
    }

    fs::write(out_repo_dir.join("README.md"), render_target_readme(&plan)?)
        .context("failed to write README.md")?;
    fs::write(
        out_repo_dir.join("pyproject.toml"),
        ["[project]", "name = \"okf-migrated-sample\"", "version = \"0.1.0\"", ""].join("\n"),
    )
    .context("failed to write pyproject.toml")?;

    let result = json!({
        "target_repo": out_repo_dir.to_string_lossy(),
        "package": package,
        "files": [
            format!("src/{package}/__init__.py"),
            format!("src/{package}/models.py"),
            format!("src/{package}/payroll.py"),
            format!("src/{package}/workflow.py"),
            "tests/test_workflow.py".to_string(),
            "README.md".to_string(),
            "pyproject.toml".to_string(),
        ],
    });
    write_json(&plan_dir.join("generated_target.json"), &result)?;
    Ok(result)
}

// ---------------------------------------------------------------------
// kl4a.codekb.transform.unique_state_contracts (lines 188-200)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.unique_state_contracts`.
///
/// De-duplicates `plan["state_contracts"]` by `(canonical_name(name),
/// target_kind)`, keeping the first occurrence of each key, in original
/// order (Python: `contract.get("name", "")`, `contract.get("target_kind",
/// "class")` as defaults when a key is absent).
pub fn unique_state_contracts(plan: &Value) -> Vec<Value> {
    let mut contracts = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let Some(arr) = plan.get("state_contracts").and_then(|v| v.as_array()) else {
        return contracts;
    };
    for contract in arr {
        let name = contract.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let key_name = canonical_name(name);
        let target_kind = contract
            .get("target_kind")
            .and_then(|v| v.as_str())
            .unwrap_or("class")
            .to_string();
        let key = (key_name, target_kind);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        contracts.push(contract.clone());
    }
    contracts
}

// ---------------------------------------------------------------------
// kl4a.codekb.transform.render_python_models (lines 203-236)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.render_python_models`.
///
/// Renders `src/<package>/models.py`: one `@dataclass` per unique state
/// contract, each with `source_name`/`source_kind` defaults copied from the
/// contract, plus an untyped `value` and a `metadata` dict field. With no
/// contracts, emits an empty `__all__: list[str] = []` module.
pub fn render_python_models(plan: &Value) -> Result<String> {
    let contracts = unique_state_contracts(plan);

    let mut lines: Vec<String> = vec![
        "\"\"\"Generated state contracts from an OKF transform plan.\"\"\"".to_string(),
        String::new(),
        "from __future__ import annotations".to_string(),
        String::new(),
        "from dataclasses import dataclass, field".to_string(),
        "from typing import Any".to_string(),
        String::new(),
    ];

    if contracts.is_empty() {
        lines.push(String::new());
        lines.push("__all__: list[str] = []".to_string());
        return Ok(lines.join("\n") + "\n");
    }

    let mut exported: Vec<String> = Vec::new();
    for contract in &contracts {
        let class_name = contract
            .get("target_name")
            .and_then(|v| v.as_str())
            .context("state contract missing target_name")?
            .to_string();
        let source_artifact_id = contract
            .get("source_artifact_id")
            .and_then(|v| v.as_str())
            .context("state contract missing source_artifact_id")?;
        let source_name = contract
            .get("name")
            .and_then(|v| v.as_str())
            .context("state contract missing name")?;
        let source_kind = contract
            .get("kind")
            .and_then(|v| v.as_str())
            .context("state contract missing kind")?;

        exported.push(class_name.clone());
        lines.push(String::new());
        lines.push("@dataclass".to_string());
        lines.push(format!("class {class_name}:"));
        lines.push(format!(
            "    \"\"\"Preserves source state contract {source_artifact_id}.\"\"\""
        ));
        lines.push(format!("    source_name: str = \"{source_name}\""));
        lines.push(format!("    source_kind: str = \"{source_kind}\""));
        lines.push("    value: Any | None = None".to_string());
        lines.push("    metadata: dict[str, Any] = field(default_factory=dict)".to_string());
    }

    lines.push(String::new());
    lines.push(format!("__all__ = {}", python_repr_str_list(&exported)));
    Ok(lines.join("\n") + "\n")
}

// ---------------------------------------------------------------------
// kl4a.codekb.transform.render_python_workflow (lines 239-258)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.render_python_workflow`.
///
/// Renders `src/<package>/workflow.py`: one function per plan operation,
/// each recording its own name into a `"visited"` list on a passed-through
/// `context` dict (a deterministic, side-effect-free stand-in workflow, not
/// a real port of the operation's original logic — the plan only carries
/// names/ids, not bodies).
pub fn render_python_workflow(plan: &Value) -> Result<String> {
    let mut lines: Vec<String> = vec![
        "\"\"\"Generated target workflow skeleton from an OKF transform plan.\"\"\"".to_string(),
        String::new(),
        "from __future__ import annotations".to_string(),
        String::new(),
    ];

    if let Some(operations) = plan.get("operations").and_then(|v| v.as_array()) {
        for operation in operations {
            let name = operation
                .get("target_name")
                .and_then(|v| v.as_str())
                .context("operation missing target_name")?;
            let source_artifact_id = operation
                .get("source_artifact_id")
                .and_then(|v| v.as_str())
                .context("operation missing source_artifact_id")?;
            lines.push(String::new());
            lines.push(format!("def {name}(context: dict | None = None) -> dict:"));
            lines.push(format!(
                "    \"\"\"Preserves source artifact {source_artifact_id}.\"\"\""
            ));
            lines.push("    state = dict(context or {})".to_string());
            lines.push(format!(
                "    state.setdefault(\"visited\", []).append(\"{name}\")"
            ));
            lines.push("    return state".to_string());
        }
    }

    Ok(lines.join("\n") + "\n")
}

// ---------------------------------------------------------------------
// kl4a.codekb.transform.render_python_payroll (lines 261-296)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.render_python_payroll`.
///
/// Renders a fixed, hand-authored `src/<package>/payroll.py` reference
/// implementation (flat 20% tax, half-up rounding to cents, CSV-ish
/// `id,hours,rate` input skipping blank lines and negative hours/rate).
/// This is static template text — `plan` is accepted (to match the other
/// `render_python_*` signatures / the call site in `generate_target_repo`)
/// but never read, exactly as in the Python source.
pub fn render_python_payroll(_plan: &Value) -> String {
    [
        "\"\"\"Generated payroll behavior from OKF sample I/O contracts.\"\"\"",
        "",
        "from __future__ import annotations",
        "",
        "from decimal import Decimal, ROUND_HALF_UP",
        "",
        "",
        "TAX_RATE = Decimal('0.20')",
        "",
        "",
        "def money(value: Decimal) -> str:",
        "    return str(value.quantize(Decimal('0.01'), rounding=ROUND_HALF_UP))",
        "",
        "",
        "def calculate_payroll(employee_id: str, hours: str, rate: str) -> str:",
        "    gross = Decimal(hours) * Decimal(rate)",
        "    tax = gross * TAX_RATE",
        "    net = gross - tax",
        "    return ','.join([employee_id, money(gross), money(tax), money(net)])",
        "",
        "",
        "def run_payroll_lines(lines: list[str]) -> list[str]:",
        "    output: list[str] = []",
        "    for line in lines:",
        "        if not line.strip():",
        "            continue",
        "        employee_id, hours, rate = [part.strip() for part in line.split(',')]",
        "        if Decimal(hours) < 0 or Decimal(rate) < 0:",
        "            continue",
        "        output.append(calculate_payroll(employee_id, hours, rate))",
        "    return output",
        "",
    ]
    .join("\n")
}

// ---------------------------------------------------------------------
// kl4a.codekb.transform.render_python_tests (lines 299-319)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.render_python_tests`.
///
/// Renders `tests/test_workflow.py`. With no operations, emits a trivial
/// always-true smoke test. Otherwise emits one test importing and calling
/// the *first* plan operation's generated workflow function and asserting
/// its name landed in `result["visited"]` (only the first operation is
/// exercised — this is the original Python's own behavior, not an
/// incomplete port).
pub fn render_python_tests(plan: &Value) -> Result<String> {
    let operations = plan
        .get("operations")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if operations.is_empty() {
        return Ok("def test_no_operations() -> None:\n    assert True\n".to_string());
    }
    let first = operations[0]
        .get("target_name")
        .and_then(|v| v.as_str())
        .context("operation missing target_name")?
        .to_string();

    Ok([
        "import sys".to_string(),
        "from pathlib import Path".to_string(),
        String::new(),
        "sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'src'))".to_string(),
        String::new(),
        format!("from migrated.workflow import {first}"),
        String::new(),
        String::new(),
        "def test_generated_workflow_operation_records_visit() -> None:".to_string(),
        format!("    result = {first}({{}})"),
        format!("    assert \"{first}\" in result[\"visited\"]"),
        String::new(),
    ]
    .join("\n"))
}

// ---------------------------------------------------------------------
// kl4a.codekb.transform.render_python_behavior_tests (lines 322-340)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.render_python_behavior_tests`.
///
/// Renders `tests/test_behavior_equivalence.py` from the *first* (and, per
/// `behavior_contracts`, currently only ever) behavior contract, embedding
/// its `input_lines`/`expected_lines` as Python list literals and asserting
/// `run_payroll_lines(input_lines) == expected_lines`. Only called by
/// `generate_target_repo` when `plan["behavior_contracts"]` is non-empty, so
/// indexing `[0]` here mirrors the Python's own unchecked
/// `plan["behavior_contracts"][0]`.
pub fn render_python_behavior_tests(plan: &Value) -> Result<String> {
    let contracts = plan
        .get("behavior_contracts")
        .and_then(|v| v.as_array())
        .context("plan missing behavior_contracts")?;
    let contract = contracts
        .first()
        .context("behavior_contracts is empty")?;

    let input_lines: Vec<String> = contract
        .get("input_lines")
        .and_then(|v| v.as_array())
        .context("behavior contract missing input_lines")?
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();
    let expected_lines: Vec<String> = contract
        .get("expected_lines")
        .and_then(|v| v.as_array())
        .context("behavior contract missing expected_lines")?
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();

    Ok([
        "import sys".to_string(),
        "from pathlib import Path".to_string(),
        String::new(),
        "sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'src'))".to_string(),
        String::new(),
        "from migrated.payroll import run_payroll_lines".to_string(),
        String::new(),
        String::new(),
        "def test_payroll_output_matches_source_sample() -> None:".to_string(),
        format!("    input_lines = {}", python_repr_str_list(&input_lines)),
        format!("    expected_lines = {}", python_repr_str_list(&expected_lines)),
        "    assert run_payroll_lines(input_lines) == expected_lines".to_string(),
        String::new(),
    ]
    .join("\n"))
}

// ---------------------------------------------------------------------
// kl4a.codekb.transform.render_target_readme (lines 343-353)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.render_target_readme`.
pub fn render_target_readme(plan: &Value) -> Result<String> {
    let source_bundle = plan
        .get("source_bundle")
        .and_then(|v| v.as_str())
        .context("plan missing source_bundle")?;
    Ok([
        "# Generated Target Repository".to_string(),
        String::new(),
        format!("Generated from source bundle `{source_bundle}`."),
        String::new(),
        "This is a deterministic skeleton for OKF target-KB and trace validation.".to_string(),
        String::new(),
    ]
    .join("\n"))
}

// ---------------------------------------------------------------------
// kl4a.codekb.transform.render_transform_plan (lines 162-185)
// ---------------------------------------------------------------------

/// Port of `kl4a.codekb.transform.render_transform_plan`.
///
/// Renders the plan-directory `README.md`: one bullet per operation
/// (source qualified name -> target name), one per unique state contract
/// (source name -> target name), one per behavior contract (name, from
/// input file, to expected file), each section falling back to a literal
/// `- None` bullet when its underlying plan list is missing/empty.
pub fn render_transform_plan(plan: &Value) -> Result<String> {
    let target = plan
        .get("target")
        .and_then(|v| v.as_str())
        .context("plan missing target")?;

    let mut lines: Vec<String> = vec![
        "# Transform Plan".to_string(),
        String::new(),
        format!("Target: `{target}`"),
        String::new(),
        "## Operations".to_string(),
    ];

    let operations = plan
        .get("operations")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for operation in &operations {
        let source_qualified_name = operation
            .get("source_qualified_name")
            .and_then(|v| v.as_str())
            .context("operation missing source_qualified_name")?;
        let target_name = operation
            .get("target_name")
            .and_then(|v| v.as_str())
            .context("operation missing target_name")?;
        lines.push(format!("- `{source_qualified_name}` -> `{target_name}`"));
    }
    if operations.is_empty() {
        lines.push("- None".to_string());
    }
    lines.push(String::new());

    lines.push("## State Contracts".to_string());
    for contract in unique_state_contracts(plan) {
        let name = contract
            .get("name")
            .and_then(|v| v.as_str())
            .context("state contract missing name")?;
        let target_name = contract
            .get("target_name")
            .and_then(|v| v.as_str())
            .context("state contract missing target_name")?;
        lines.push(format!("- `{name}` -> `{target_name}`"));
    }
    // Python checks truthiness of the RAW plan["state_contracts"], not the
    // deduplicated list, so an empty raw list still gets "- None" even if
    // (impossible in practice, but mirrored anyway) dedup somehow differed.
    let state_contracts_empty = plan
        .get("state_contracts")
        .and_then(|v| v.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(true);
    if state_contracts_empty {
        lines.push("- None".to_string());
    }
    lines.push(String::new());

    lines.push("## Behavior Contracts".to_string());
    let behavior_contracts_arr = plan
        .get("behavior_contracts")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for contract in &behavior_contracts_arr {
        let name = contract
            .get("name")
            .and_then(|v| v.as_str())
            .context("behavior contract missing name")?;
        let input_name = contract
            .get("input_name")
            .and_then(|v| v.as_str())
            .context("behavior contract missing input_name")?;
        let expected_name = contract
            .get("expected_name")
            .and_then(|v| v.as_str())
            .context("behavior contract missing expected_name")?;
        lines.push(format!("- `{name}` from `{input_name}` -> `{expected_name}`"));
    }
    if behavior_contracts_arr.is_empty() {
        lines.push("- None".to_string());
    }
    lines.push(String::new());

    Ok(lines.join("\n"))
}

// ---------------------------------------------------------------------
// Helpers (not present as standalone Python symbols; needed for fidelity)
// ---------------------------------------------------------------------

/// Approximates CPython's `repr()` for a single `str`.
///
/// UNCONFIRMED / not independently verified against CPython byte-for-byte:
/// picks `"` as the quote character only when the string contains `'` and
/// no `"` (matching CPython's own quote-selection rule), escapes `\\`, the
/// chosen quote character, `\n`/`\r`/`\t`, and any other C0 control
/// character (`< 0x20` or `0x7f`) as `\xHH`; every other character —
/// including non-ASCII — is emitted literally, which matches CPython for
/// printable Unicode but not for other Unicode categories CPython also
/// escapes (e.g. some non-printable non-Latin control/format characters).
/// This matters because `render_python_behavior_tests` embeds *arbitrary*
/// on-disk testdata (`employees.in` / `expected_payroll.out` content, not
/// just generated identifiers) as Python list-of-str literals: a behavior
/// contract fixture containing an unusual Unicode control character could
/// produce Python source that differs from what CPython's own `repr()`
/// would have produced. Verify this against real behavior-contract fixtures
/// via `cross-lang-fidelity-check` before treating generated behavior-test
/// files as guaranteed-valid for arbitrary input.
fn python_repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || (c as u32) == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Approximates CPython's `repr()` for a `list[str]` (Python's `!r` format
/// spec applied to a list), e.g. `['a', 'b']`. See `python_repr_str` for the
/// per-element caveats.
fn python_repr_str_list(items: &[String]) -> String {
    let inner = items
        .iter()
        .map(|s| python_repr_str(s))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{inner}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Direct ports of the pure-string-transform behavior described in the
    // Python source excerpts. No test files for these symbols were found via
    // `code_tests_for_symbol` (transform.py's plan/repo generation is
    // apparently only exercised indirectly, at the CLI level, by
    // `kl4a/tests/codekb/test_code_kb_cobol_transformation.py` — a fuller
    // fixture-based behavioral diff against that Python test belongs in the
    // Phase 3 cross-lang-fidelity-check pass, not here).

    #[test]
    fn python_identifier_basic() {
        assert_eq!(python_identifier("Calculate Payroll"), "calculate_payroll");
        // NOTE: `slugify("")` (kl4a.kl4a.ids.slugify) falls back to "item"
        // internally (`return slug or "item"`), so it never actually hands
        // `python_identifier` an empty string — its own `if candidate.is_empty()`
        // "migrated_operation" fallback is therefore dead code given
        // slugify's documented contract, reachable only if that contract
        // ever changes. Asserting the real end-to-end result here, not the
        // unreachable branch.
        assert_eq!(python_identifier(""), "item");
        assert_eq!(python_identifier("100-days"), "op_100_days");
    }

    #[test]
    fn python_class_name_basic() {
        assert_eq!(python_class_name("calculate payroll"), "CalculatePayroll");
        // See the note in `python_identifier_basic`: slugify("") == "item",
        // so the "MigratedRecord" fallback is likewise unreachable here.
        assert_eq!(python_class_name(""), "Item");
        assert_eq!(python_class_name("100 days"), "Record100Days");
    }

    #[test]
    fn repr_str_quote_selection() {
        assert_eq!(python_repr_str("plain"), "'plain'");
        assert_eq!(python_repr_str("it's"), "\"it's\"");
        assert_eq!(python_repr_str("say \"hi\""), "'say \"hi\"'");
        assert_eq!(python_repr_str("both ' and \""), "'both \\' and \"'");
    }
}
