---
title: Release Validation Checklist
description: >-
  Repeatable installation and first-use checks for the current KL4A release.
---

# Release Validation Checklist

Use this checklist when validating a KL4A release on a clean machine or with
an external evaluator. Record the platform, asset name, command output, and
any blocker for every run. A successful check means a new user can complete a
useful first task, not merely download a file.

## Release under test

- **Release:** [KL4A 0.0.2-alpha](https://github.com/CogniSwitch/KL4A/releases/tag/0.0.2-alpha)
- **Desktop workflow:** SOP Knowledge Bundles
- **CLI/MCP workflows:** SOP Knowledge Bundles and Code Knowledge Bundles
- **CodeKB parsing scope in this release:** Python and COBOL. Other languages
  are inventoried and reported as unsupported; they do not produce parsed
  modules or symbols.

## Current verification record

| Check | Windows | macOS | Linux |
|---|---|---|---|
| Download resolves to the published asset | Verified | To verify | To verify |
| `kl4a --use codekb --help` exits successfully | Verified | To verify | To verify |
| Build, validate, and query a Python CodeKB bundle | Verified | To verify | To verify |
| SOPKB desktop install and first bundle | To verify | To verify | To verify |
| SOPKB MCP connection from an MCP client | To verify | To verify | To verify |

The Windows CLI verification was run against the published
`kl4a-windows-x86_64.exe` asset on 12 October 2026. The static CodeKB run over
the repository's Python benchmark scripts completed with 3 sources, 3 modules,
18 symbols, 105 relations, 3 knowledge items, and 0 validation errors.

## CodeKB: first successful run

1. Download the platform-specific `kl4a` asset from the
   [latest release](https://github.com/CogniSwitch/KL4A/releases/latest).
2. On Windows, rename it to `kl4a.exe` if desired and place its folder on
   `PATH`. On macOS or Linux, make it executable:

   ```bash
   chmod +x kl4a-<platform>
   ```

3. Confirm that the CodeKB commands are available:

   ```bash
   kl4a --use codekb --help
   ```

4. Build a bundle from a supported repository. Start with static mining; it is
   deterministic and does not need an LLM provider:

   ```bash
   kl4a --use codekb build /absolute/path/to/python-or-cobol-repository \
     --bundle ./my-code-bundle --mining static
   ```

5. Validate the bundle. Zero errors is the success criterion; unresolved
   external relations may be reported as warnings:

   ```bash
   kl4a --use codekb validate ./my-code-bundle
   ```

6. Confirm the bundle produces evidence-backed task context:

   ```bash
   kl4a --use codekb context ./my-code-bundle \
     --task explain --query "authentication flow"
   ```

   Expected result: JSON containing code knowledge, evidence, symbols or
   relations relevant to the query. If the output is empty, first confirm that
   the repository contains supported source files.

7. Optional: connect the completed bundle to an MCP-capable client using:

   ```json
   {
     "mcpServers": {
       "codekb": {
         "command": "kl4a",
         "args": ["--use", "codekb", "mcp", "serve", "/absolute/path/to/my-code-bundle"]
       }
     }
   }
   ```

## SOPKB desktop: first successful run

1. Download the desktop asset for the platform from the
   [latest release](https://github.com/CogniSwitch/KL4A/releases/latest).
2. Complete the platform's first-launch security prompt if shown. The current
   builds are not code-signed.
3. Create a bundle and import a real Markdown, DOCX, or PDF policy/procedure.
4. Confirm the application produces sections, evidence-linked knowledge, and
   reviewable decisions.
5. Export or query the resulting bundle, then record the outcome and any
   point where a first-time user needed assistance.

See the [SOPKB Quickstart](sopkb/quickstart.md) and
[Desktop UI Guide](sopkb/DESKTOP_UI_GUIDE.md) for the detailed workflow.

## What to capture from every evaluator

- Platform, OS version, and asset filename.
- Whether installation and first launch succeeded without maintainer help.
- Repository or SOP input type, excluding any confidential source content.
- Commands run, exit codes, error output, and validation-error count.
- Whether the evaluator could use the resulting bundle in one real task.
- One concrete improvement that would make the next attempt easier.

## Escalation criteria

File an issue before recruiting more evaluators if any of these occur:

- A documented download URL does not resolve to its named asset.
- A supported Python or COBOL repository produces no sources without an
  explicit unsupported-language warning.
- `validate` reports an error for the documented happy path.
- An MCP client cannot start the documented stdio command.
- A new SOPKB desktop user cannot create and inspect a first bundle.
