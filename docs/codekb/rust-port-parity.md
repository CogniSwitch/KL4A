---
title: Rust Port Parity
description: >-
  An audit of a Rust port of a Python CLI application, covering two of its
  knowledge-extraction tools, against the Python reference implementation - groundwork
  for a planned web UI built on the port. What matches, what doesn't, and what wasn't
  verified.
---

# Python to Rust functional parity

Date: 2026-10-05

!!! info "Why this audit exists"
    This Rust port is being evaluated as the foundation for a roadmap item: a web UI
    that surfaces API endpoints, codebase architecture, and execution flow for the
    application's two knowledge-extraction tools, built on the ported implementation
    rather than the Python one. Before that work builds on it, this audit establishes
    where the port currently matches the Python reference and where it doesn't (see the
    [findings](#4-findings) below) - it is not yet built, packaged, or distributed as
    part of any release, and its output is not interchangeable with the Python
    implementation's output.

Does the Rust port behave like the Python reference implementation? For well-formed input it mostly does. It is not a full match: generated output content differs for Python sources, and the default config file is found through a build-time path.

| | |
|---|---|
| **Reference** | the Python reference implementation (working tree) |
| **Port** | the Rust port (covers two of the application's tools) |
| **Scope** | Two knowledge-extraction tools only - called **Tool A** (source code) and **Tool B** (API specs) below. Excluded: a third tool (SOP documents), the shared CLI dispatcher, HTTP and HTTPS (web server, LLM transport) |
| **Platform** | Windows 11, Python 3.12, Rust debug build |

### Summary

| 🔴 Critical | 🟠 High | 🟡 Medium | ⚪ Low | Areas compared |
|:---:|:---:|:---:|:---:|:---:|
| **0** | **2** | **12** | **4** | **4** |

<sub>Counts are finding rows in section 4. Minor differences that do not affect behaviour are listed separately in the [note at the end](#note-on-minor-differences).</sub>

> [!NOTE]
> **How to read this.** Four agents ran identical inputs through both implementations and saved the outputs. Each finding cites those saved runs. The coordinating session did not re-run the probes. Anything not executed is marked static-only or unverified. Nothing ran on Linux or macOS.

## Contents

1. [Executive summary](#1-executive-summary)
2. [Coverage against the audit parameters](#2-coverage-against-the-audit-parameters)
3. [Architecture mapping](#3-architecture-mapping)
4. [Findings](#4-findings)
5. [Same in both (not parity issues)](#5-same-in-both-not-parity-issues)
6. [Missing and additional functionality](#6-missing-and-additional-functionality)
7. [Recommended Rust fixes](#7-recommended-rust-fixes)
8. [Unverified](#8-unverified)
9. [Conclusion](#9-conclusion)

---

## 1. Executive summary

### ✅ Matches Python

- CLI surface: 23 Tool A and 15 Tool B subcommands, same flags and exit codes
- Output file layout, counts, IDs, ordering
- Relations, entrypoints, procedures on shared fixtures
- `validate` rules, messages, exit codes (45 mutated bundles)
- Lifecycle merge, trace output
- MCP tool catalogues and valid-call results (9 Tool A, 10 Tool B tools)
- LLM settings precedence and prompt bodies

### ❌ Does not match

- Default config file found through a build-time path
- Python signatures, docstrings, stdlib detection and test classification are computed differently
- YAML scalars, name templates, some CLI options
- Some malformed input is accepted silently, including a false pass in `validate`

### ❔ Not verified

- Linux and macOS behaviour
- Concurrency, the `superseded` lifecycle state
- Performance (Rust was a debug build)

---

## 2. Coverage against the audit parameters

Each row is one dimension from the audit instructions. **Executed** means the same input was run through both implementations and the output saved. **Partial** means some of it was executed. **Not verified** means no execution evidence. Source reading alone never counts as covered.

### What each parameter checks

The application is a CLI with a shared core (config, settings, bundle state) and two
knowledge-extraction tools built on top of it: **Tool A**, which turns a source-code
repository into a knowledge bundle (symbols, relations, architecture, mined claims),
and **Tool B**, which does the same for a set of API specs (operations, schemas,
capability cards). Each tool exposes the same shape of surface: a CLI, a pipeline that
transforms input into a bundle on disk, and a read-only MCP server that lets an agent
query that bundle. The quartet below is why this audit exists - a planned web UI reads
through that MCP surface, so a gap found here is a gap that UI would inherit.

| Parameter | What it actually checks |
|---|---|
| Entry points: CLI | The command-line surface each tool exposes - every subcommand (build, scan, parse, validate, …), its flags, defaults, and exit code. This is how a human or script drives either tool directly; a flag that silently behaves differently between implementations breaks any script written against one of them. |
| Entry points: MCP | The second, machine-facing surface: a read-only JSON-RPC server per tool that an agent (or the planned web UI) queries instead of shelling out to the CLI. Checks the tool catalogue itself and the protocol's own edge behavior (malformed lines, odd request IDs, notifications), not just individual tool calls. |
| Input and output parity | Whether the actual knowledge bundle produced from the same repository/spec is structurally the same - directory tree, JSON/YAML content - under both normal and adversarial input (empty, null, unicode, BOM, oversized). Confirms the two implementations don't just exit the same way, but produce equivalent knowledge. |
| Error and exception parity | Feeds both implementations the same corrupted bundle states and malformed configs and compares how each fails: same exit code, same validation message, same point of failure. Matters because anything consuming either tool's output may branch on the specific error shape. |
| Edge cases | Boundary conditions in how the pipeline's input-handling assumptions hold up: empty repos, duplicate files, unicode filenames, rebuilding over existing output, the source repo changing between builds - the unusual-but-plausible cases a real repository produces, as opposed to synthetic fixtures. |
| Data transformation pipeline | The actual stage-by-stage sequence each tool runs input through - for Tool A: scan, parse, relations, entrypoints, architecture detection, procedure mining, render, validate. Checks that each stage's output, and the final symbol/relation/knowledge-item counts, match end to end on a real, non-trivial codebase, not only on small fixtures. |
| Configuration and environment | The shared core's config layer: which config filename is picked up, the precedence between a saved setting, an environment variable, and a `.env` file, and how skip-list and naming-template options are interpreted. A silent disagreement here misconfigures a build without any visible error. |
| External integrations: filesystem, git | How each tool actually writes its bundle to disk and what git metadata (commit, branch) it records about the repository it analyzed - confirms both implementations touch the filesystem and git the same way, not just that their in-memory logic agrees. |
| Hybrid mining (LLM prompts and cache) | An optional mode where, beyond static analysis, extracted symbols are sent to an LLM to enrich the mined knowledge, with responses cached so a rerun doesn't re-call the model. Checks prompt bodies, accept/reject counts, cache-hit behavior, and handling of a corrupted cache - the one place the pipeline talks to an external system rather than just parsing input. |
| Ordering, filtering, dedup, determinism | Whether identifiers, sort order, and deduplication of files/operations are computed the same way, and whether rerunning on unchanged input reproduces the same bundle (timestamps aside). Matters for anything that diffs or caches bundle output across runs. |
| Concurrency and async | Whether either implementation stays correct when a build and a read happen at once, or recovers cleanly when a build crashes mid-run. Not executed in this audit - a gap, not a pass. |
| File and artifact generation | The literal on-disk shape of a bundle: file naming, directory layout, counts, front matter, manifests, reports, trace directories - the thing everything downstream (the MCP server, the planned web UI) actually reads from. |
| API and protocol parity (per operation) | For every individual MCP tool call, not just the catalogue listing - do parameters, defaults, schemas, results, and error behavior match? This is the parameter that most directly matters for the planned web UI: it would be built against one implementation's MCP surface and needs each operation to behave identically to the other's. |
| Security and permissions | Whether either implementation has any authentication/access-control code (neither does - both are local, unauthenticated tools), how each handles permission-denied files and path-traversal attempts (`..`, absolute paths), and whether API keys ever leak into logs, cache, or output. Not about access control existing, but about not introducing an information leak or an inconsistent failure mode. |
| Existing tests | Whether the Python implementation's own test suite, and the Rust port's `cargo test`, actually pass. This audit reused the Python suite's *fixtures* as inputs but didn't run either suite - flagged as something that should happen before either implementation is treated as validated. |

| Audit parameter | Coverage | What was done | What is missing |
|---|---|---|---|
| Entry points: CLI | 🟢 Executed | All Tool A and Tool B subcommands, flags, defaults, exit codes, usage errors compared. Wording differs (argparse vs clap); exit codes match. | None within scope. |
| Entry points: MCP | 🟢 Executed | tools/list, every tool call on several bundles, protocol edge cases (initialize, ping, notifications, malformed lines, odd IDs). About 70 calls per tool set for Tool A and 84 requests for Tool B. | None reported within the calls made. |
| Input and output parity | 🟢 Executed | Output trees, JSON and YAML compared by structure. Empty, null, unicode, BOM, oversized (5 MB query, 100 KB line) inputs used. | Byte-level output was not treated as the test; key order, escaping and newline differences are listed in the [note at the end](#note-on-minor-differences). |
| Error and exception parity | 🟢 Executed | About 33 Tool B bundle corruptions, 20 Tool A state variants, 16 trace shapes, 45 validate mutations. | A panic in the Rust pipeline worker is static-only. |
| Edge cases | 🟡 Partial | Empty, missing, malformed, duplicate, unicode, repeated runs, already-existing output, changed repo on rebuild. | Permission failures, symlinks, interrupted execution, timeouts, very large repos, case-insensitive file system effects. |
| Data transformation pipeline | 🟢 Executed | Traced through scan, parse, relations, entrypoints, architecture, procedures, render, validate on four repo fixtures plus Python and COBOL edge repos. Then a full build of a real 134-file Python repo (a copy of the reference implementation's own source tree) on both sides. Counts matched exactly: 142 sources, 140 modules, 1857 symbols, 12809 relations, 1232 knowledge items, 0 validation errors, 7937 warnings. Nothing missing in Rust; 6 extra dependency pages in Rust (stdlib list difference). 1035 of 18493 files differ in content, all traced to already-listed differences (signatures, docstrings, stdlib dependencies, handler line). A second Rust build over the first changed nothing. | Rust ran as a debug build (631 s first build, 536 s second), so speed was not compared. Python was not separately timed on this run. |
| Configuration and environment | 🟢 Executed | Filename precedence, wrapper key, skip_dirs, templates, mining options, env and .env precedence, malformed and null values, settings paths. | Behaviour on Linux and macOS (HOME handling, paths). |
| External integrations: filesystem, git | 🟢 Executed | Output writes and git commit and branch metadata matched. | Concurrent reader and writer race. |
| Hybrid mining (LLM prompts and cache) | 🟡 Partial | Prompt bodies, accepted and rejected counts, cache hits on rerun, corrupt cache handling compared against a local mock server. Transport (HTTP and HTTPS) is out of scope. | The author and validate step against a real provider. |
| Ordering, filtering, dedup, determinism | 🟢 Executed | IDs, sort orders, dedup of operations and files, result ordering in queries compared. Timestamps ignored as noise. | POSIX sort order for Tool B's scan. |
| Concurrency and async | 🔴 Not verified | Nothing executed within scope. | Concurrent builds, reader and writer file races, atomic-write retry, worker failure handling. |
| File and artifact generation | 🟢 Executed | File names, layout, counts, front matter, manifests, reports, trace directories. | File permissions. |
| API and protocol parity (per operation) | 🟢 Executed | Tool names, parameters, defaults, schemas, results and error behaviour for all 9 Tool A and 10 Tool B tools. | None found beyond the coercion edge cases already listed. |
| Security and permissions | 🟢 Executed | Searched both trees for authentication and access-check code and found none on either side; the MCP surface lists only read tools and an unlisted tool is refused on both. Path inputs (about 20 cases) and file permissions (write-denied, read-only, unreadable and locked files, about 15 cases) run identically on both: same exit codes, same abort point, same files left behind. A fake API key never appeared in stdout, stderr, output, cache or home directory on either side (keys read from env, .env and saved settings). Differences: a directory junction pointing outside the repo is followed by Python and skipped by Rust, and error text differs. | POSIX permissions and file symlinks (Windows only; junctions used). Not every command was run for path and permission cases. The reason Rust skips junctions was not investigated. |
| Existing tests | 🔴 Not verified | Fixtures from the Python test suite were reused as inputs. | Neither the Python test suite nor `cargo test` was reported as run. This should be done before sign-off. |

### By category (from the instructions)

- **A. Functional parity:** CLI surface, layout, IDs, validate, lifecycle merge, MCP catalogues, settings precedence.
- **B. Behavioral difference:** signatures, docstrings, stdlib detection, YAML scalars, name templates, `diff --capability-id`, empty CLI options.
- **C. Missing functionality:** Python-equivalent signature, docstring and stdlib logic; embedded default config.
- **D. Additional functionality:** atomic-write helper, timestamp quoting, more lenient parsing.
- **E. Edge-case difference:** argument coercion, JSON parser extensions, parse acceptance (BOM, NUL, depth).
- **F. Error-handling difference:** malformed state, JSON-RPC lines.
- **G. Environment or runtime:** `\\?\` paths, HOME vs USERPROFILE, scan order.
- **H. Unverified:** see [section 8](#8-unverified).

### Reading the coverage

- Eleven of fifteen parameters are executed, two are partial, two are not verified.
- The weakest areas are concurrency, hybrid mining with real model output, and the existing test suites.
- The strongest are the MCP, CLI and output-layout comparisons.
- Everything ran on Windows only.

---

## 3. Architecture mapping

| Python | Rust | Status |
|---|---|---|
| Tool A's source tree: pipeline, parse, adapters, relations, entrypoints, architecture, procedures, render, mcp, server, web, context, agent, trace, lifecycle, state | The Rust port's equivalent crate, same module names | 🟡 Partly different |
| Tool B's source tree | The Rust port's equivalent crate | 🟡 Partly different |
| PyYAML, `ast`, argparse | serde_yaml, rustpython-parser, clap | 🟢 Runtime difference |

---

## 4. Findings

ID prefix: **B** build pipeline (Tool A), **R** runtime/MCP/web, **A** Tool B, **C** core/config. Evidence paths are relative to `scratchpad/parity/`.

### 🟠 High

| ID | Area | Python | Rust | Evidence |
|---|---|---|---|---|
| B-F1 | Python signatures | `ast.unparse` | Own unparser: `b = 1` vs `b=1`, `dict[(str, str)]`, lambda loses parameters, comprehensions become a placeholder. Changes symbols, pages, LLM request bodies, cache keys. | `build/out/build_python_procedure_repo`, `build_sig_py`, `hyb_proc` |
| C-F3 | Default config path | Yaml shipped as package data | Found through `CARGO_MANIFEST_DIR` at runtime. If the file moves, build exits 1 with no path in the error. | `core/runs/cfgpath` (scratch copy) |

### 🟡 Medium

| ID | Area | Python | Rust | Evidence |
|---|---|---|---|---|
| B-F2 | Docstrings | `ast.get_docstring` (cleaned) | Raw string. Changes docstring fields, claim text, pages. | `build/out/build_syn_py`, `build_sig_py` |
| B-F3 | Accepted Python source | NUL byte, 300-deep parens, BOM give parse_error; PEP 701 f-string accepted | Opposite in each case. BOM file adds extra module, symbol and evidence files. Error text and line field differ. | `build/out/build_syn_py`, `build_edge_py2`, `build_edge_py3` |
| B-F5 | Stdlib detection | `sys.stdlib_module_names` | About 60 hand-listed names. All stdlib plus 3 third-party imports: 3 dependencies vs 160; spurious `__future__` dependency. | `build/out/build_stdlib_py`, `build_sig_py` |
| B-F6 | Test classification | `"test" in path.parts` of absolute path | Different path check. Repo root named `test`: kind test vs source, changing mined claims. | `build/out/build_test` |
| C-F7 / B-F7 | Output name template | `str.format`: unknown `{foo}` exits 1; format specs apply | Plain replace: `{foo}` stays literal (exit 0); a spec gives an invalid directory (exit 1). | `core/runs/c04`, `c05`; `build/out/cfg_bundle_tmpl_bad` |
| C-F5 / A-F3 | YAML 1.1 vs 1.2 | yes/no/on/off are booleans, octal ints, dates are dates. A date in a Tool B spec crashes parse. | Strings. `procedures.enabled: off` stays true (64 vs 63 files). Unquoted yes/no/on written out. | `core/runs/c36`; Tool B `y_bool`, `y_date` |
| A-F4 | Tool B IDs | Unquoted YAML status `200`: hashed as integer | Hashed as string. Response and evidence IDs and order differ. Quoted codes and JSON match. | Tool B `y_intstatus`, `y_floatver` |
| A-F2 | Tool B `diff --capability-id` | Key is operationId or `METHOD:path`; real or bogus ID exits 1 | CLI never forwards the flag (opposite results); MCP keeps Python behaviour, so Rust CLI and MCP disagree. | Tool B `diff_capid_*` |
| R-F5 | Tool A empty options | Empty `--query` and `--language` mean unset | Used as values. context: 11 vs 2 symbols; `--language ""` returns 0. MCP unaffected. | runtime `cli_ctx_qempty`, `cli_*_lang_empty` |
| R-F9 | `trace review` without `--target` | Clears targets to `[]` | Keeps existing targets | runtime `trace_rereview_py`, `_rs` |
| C-F6 / C-F8 / A-F16 / R-F10 | Malformed state or config | Many shapes crash (exit 1) | Tolerated or own message. `code_knowledge` items as dict: false pass with 0 errors. `skip_dirs` null: Rust exits 1, Python builds. | `core/runs/v_state_*`, `c10`, `c26`, `c27`; runtime `var_*`; Tool B `mut_n3`, `m19` |
| S-1 | Directory junctions (Tool A build, Tool B scan) | Follows a junction that points outside the repo: 5 sources, 4 modules (Tool A); 3 sources (Tool B) | Skips it: 4 sources, 3 modules (Tool A); 2 sources (Tool B). Cause not investigated. | `security/runs/c_symlink_junction_build`, `a_scan_junction` |

### ⚪ Low

| ID | Area | Python | Rust | Evidence |
|---|---|---|---|---|
| C-F10 | Settings directory | `USERPROFILE` on Windows | `HOME` first. When they differ, saved settings are ignored. | `core/out/llm/home_divergence` |
| C-F11 / R-F15 / B-F12 / A-F12 | Windows paths | Normal drive paths | `\\?\C:\` prefix in messages and stored `repo_dir`; mixed separators; malformed Tool B index IDs | core `c02`, `i3`; runtime `pipeline_run_*.json` |
| A-F13 | Tool B scan order | Case-insensitive on Windows | Case-sensitive | Tool B `order/api_inventory.json` |
| C-F12 | Pipeline worker failure | Error is `ValueError: msg` with traceback | Bare message. Static only: a panic would leave status running. | `core/runs/rs_fail_skipdirs` |

---

## 5. Same in both (not parity issues)

- Tool B keeps a stale canonical index: a later `build` does not refresh it, so reads show old state.
- All Tool B cards are stamped `review_status: validated`.
- Config keys `language_detection.*` and `languages.*.enabled|file_extensions|adapter|semantics` are read by no Python code, and are inert in both.
- A non-UTF-8 Python source aborts the whole build in both, with different messages.
- No authentication or access-check code was found in either implementation. The search patterns and every hit are in the security agent's report. The MCP tool lists match (9 Tool A, 10 Tool B), and an unlisted tool such as `code.build` returns "unknown tool" on both.
- Neither rejects `..` or absolute output paths, and MCP `api.changes.impact` reads any directory given to it on both sides.
- Both abort on the first failing write, leave no temporary file behind, and write API keys only to the saved-settings file in plain text (static reading; that function has no callers in either tool).

---

## 6. Missing and additional functionality

### Missing in Rust

- Embedded default config (Rust reads it from a build-time path)
- Python-equivalent `ast.unparse`, `get_docstring` and `sys.stdlib_module_names` behaviour

### Added in Rust

- `Recorder` trait, atomic-write helper, timestamp quoting
- Checked-in copy of the default config under the Rust port's Tool A crate
- More lenient handling of malformed state, which changes error behaviour

---

## 7. Recommended Rust fixes

1. **Config path.** Embed the default yaml with `include_str!` instead of `CARGO_MANIFEST_DIR`.
2. **Python parsing.** Match `ast.unparse` output, `get_docstring` cleaning, the full stdlib list, the `test` path check, and CPython acceptance of BOM, NUL, nesting depth and PEP 701 f-strings.
3. **Tool B.** Forward `--capability-id` in CLI diff with Python semantics; hash YAML integer keys as integers.
4. **CLI options.** Treat empty `--query` and `--language` as unset; decide and align `trace review` without `--target`; mirror Python `int()` coercion for depth and tier.
5. **Name templates.** Implement `str.format` behaviour or reject unknown placeholders with exit 1.
6. **YAML policy.** Decide on YAML 1.1 versus 1.2 explicitly; quote ambiguous scalars on write.
7. **Junctions.** Decide whether directory junctions are followed during scan, and make both tools do what the Python reference does.
8. **Paths and workers.** Strip the `\\?\` prefix from displayed and stored paths; catch panics in the pipeline worker so run state resets.

---

## 8. Unverified

<details>
<summary>Show the full list</summary>

- LLM author step against a real provider; only a local mock server was used.
- Lifecycle `superseded` transition; `write_text_atomic` retry; pipeline worker concurrency and panic handling.
- Existing Python tests and `cargo test` were not run.
- POSIX file permissions and file symlinks (Windows only; junctions were used). Why Rust skips junctions. Config `default_bundle_dir` with an absolute path (read statically, not run).
- js, ts, java, go, rs adapters: detected but skipped in both, warnings only.
- Tool B config and lifecycle beyond the mutation matrix; OpenAPI 3 beyond validate; external `$ref` wording; symlinks, permissions, non-UTF-8 specs.
- Linux and macOS. The `\\?\` path, HOME/USERPROFILE and scan-order findings are Windows-specific.

</details>

---

## 9. Conclusion

For both tools, on well-formed input, the port has substantial parity. It is not full parity. Generated output content for Python sources (signatures, docstrings, dependencies) differs, so output from the two implementations is not interchangeable. Most remaining differences are edge cases and malformed-input handling.

---

## Note on minor differences

> [!NOTE]
> The differences below were observed during the audit but are not treated as parity issues. They do not change behaviour that users or scripts depend on: exit codes match, parsed content is equal, or the case only arises with invalid or exotic input. In several of them Python's behaviour is a quirk rather than intended behaviour (for example crashing on a malformed JSON-RPC line, or printing `None`), so the Rust port does not need to reproduce it. They are kept here for traceability.
>
> Two are cheap to address if byte-level compatibility becomes a goal: key order (serde_json `preserve_order` and ordered YAML output) and more specific OS error text.

<details>
<summary>Show the minor differences</summary>

| ID | Area | Python | Rust | Evidence |
|---|---|---|---|---|
| C-F4 | Key order | Insertion order | Alphabetical. 108 of 126 generated files differ in bytes, equal once parsed. | `core/runs/base_pysimple`; Tool B `out payments_json` |
| A-F8 / R-F6 / R-F7 / C-F14 | MCP and JSON edge cases | Non-object line kills the server (exit 1); NaN, lone surrogate, big integer IDs accepted | Error reply, keeps serving, exit 0; NaN and surrogate exit 1; big ID becomes a float | runtime `raw_*`; Tool B `mcp/s_*` |
| R-F3 / R-F4 / A-F9 / R-F11 | Argument coercion | `int()` and truthy coercion; tier `"4"` holds the gate | Non-strings dropped; depth 0 echoed as 0; tier `"4"` counts as 0 so tier-4 claims pass the gate (schema-violating state only) | runtime tool-call probes ids 12, 38, 42-50; `var_tier4_agent` |
| S-2 | Permission and path error text | Names the OS reason, e.g. `[Errno 13] Permission denied`, `[WinError 5] Access is denied` | Same exit codes and same files left behind, but less specific text: `os error 183` for a denied write, `reading <path>` with no reason for unreadable files | `security/runs/p_*`, `pa_*`, `c_build_bundle_is_file` |
| B-F10 | Render | Unresolved argparse handler prints `None` | Prints empty string | `build/out/build_arch_py` |
| B-F13 | Hybrid cache | Python `json.dumps` fingerprint | serde fingerprint. Cache files not interchangeable. | `build/out/hyb_simple` |
| A-F14 | `swagger2_parse.json` | Evidence path by fact ID; parameters omitted when empty | Evidence path by hash; always `parameters: []` | Tool B `scompare` |
| R-F1 / R-F2 / C-F16 / A-F6 / A-F10 / C-F17 | Bytes and wording | Escapes non-ASCII, quoted KeyError text, argparse and PyYAML messages, spaced JSON | Raw UTF-8, unquoted, clap and serde messages, compact JSON. Parsed content equal. | see agent reports |
| C-F13 / C-F15 / C-F18 | Id, JSON and knowledge-edge encoding | `bounded_id` tiny lengths, `splitlines` on `\v` `\f` `\u2028`, bool to `True` | Differ in each case | core reports |

</details>
