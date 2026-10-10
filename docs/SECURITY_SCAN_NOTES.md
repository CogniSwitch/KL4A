---
title: Security Scan Notes (Pre-Public Hygiene Pass)
---

# Security Scan Notes (Pre-Public Hygiene Pass)

Date: 2026-08-07

This document records a manual secret-scan pass performed on this repository ahead of making it public, since no dedicated secret-scanning tool was available in the environment used for the pass. It is not a substitute for running a real scanner, and a real scanner should still gate the transition to public (see Recommendation below).

## Tooling availability

!!! info "Standard secret-scanning tools were not available in this environment"
    The following were checked and confirmed not installed/runnable:

    ```text
    command -v gitleaks   -> not found
    command -v trufflehog  -> not found
    command -v ggshield    -> not found
    ```

Because none of the standard scanners were available, the checks below were done manually with `git log` and `grep`/ripgrep against both the full commit history (across all local branches and refs) and the current working tree.

## Scope

- Full history across all refs: `git rev-list --all` — at the time of this scan there were **7 commits total** across all local branches and remotes (`main`, `dev`, and all `feat/*` branches), so a full manual sweep of every historical diff was feasible.
- Branches present at scan time: `main`, `dev`, `feat/ci-cd-quality-gates`, `feat/community-governance`, `feat/docs-set`, `feat/github-repo-config`, `feat/launch-checklist`, `feat/legal-licensing`, `feat/packaging-distribution`, `feat/post-launch-ops`, `feat/release-process`, `feat/roadmap`, `feat/security-hygiene`, plus local worktree branches. `git log --all` walks the union of history reachable from all of these.
- Current working tree: `git ls-files` + `git grep` over everything currently checked in.

## Commands run

??? note "Exact patterns and commands used for each check"
    Against full history (`git log --all -p | grep -inE '<pattern>'`):

    1. AWS access key IDs: `AKIA[0-9A-Z]{16}`
    2. Private key headers: `BEGIN (RSA |EC |DSA |OPENSSH |)PRIVATE KEY`
    3. Generic credential assignments: `(password|passwd|secret|token|apikey|api_key|access_key|client_secret)\s*[:=]\s*['"][A-Za-z0-9+/=_\-]{8,}['"]`
    4. Connection strings with embedded credentials: `(postgres|postgresql|mysql|mongodb|redis|amqp)://[^:\s]+:[^@\s]+@`
    5. Common vendor token prefixes: `sk-[A-Za-z0-9]{16,}`, `ghp_[A-Za-z0-9]{20,}`, `xox[baprs]-[A-Za-z0-9-]{10,}`, `Bearer [A-Za-z0-9._\-]{20,}`
    6. Any `.env`/`.env.*` files ever added: `git log --all --diff-filter=A --name-only | grep -inE '\.env($|\.)'`
    7. Every occurrence of `AZURE_OPENAI[A-Z_]*` (to inspect specifically whether any are assigned literal values rather than read from the environment)
    8. Every occurrence of `AZURE_OPENAI_API_KEY` assigned directly to a quoted literal (as opposed to an `env(...)`/`os.environ.get(...)` lookup)
    9. Every occurrence of `127.0.0.1` and `localhost`

    Against the current working tree (`git grep -inE '<pattern>' -- .` and `git ls-files | grep`):

    10. Patterns 1, 3, and 4 above, repeated against tracked files as they exist today.
    11. Filenames with sensitive-looking extensions: `\.(pem|key|p12|pfx|env)$`

## Findings

!!! success "No secrets found in history or working tree"
    - No AWS access key IDs, private key headers, generic password/secret/token/apikey literal assignments, credential-bearing connection strings, or common vendor token prefixes (`sk-`, `ghp_`, `xox*`, bearer tokens) were found anywhere in history or in the working tree. All pattern searches above returned zero matches.
    - No `.env` or `.env.*` file has ever been committed to any branch.
    - No files with sensitive extensions (`.pem`, `.key`, `.p12`, `.pfx`, `.env`) are tracked in the working tree.
    - `AZURE_OPENAI_*` occurs 30+ times across history and the working tree, entirely as environment variable names — every occurrence is either an `env("AZURE_OPENAI_...")` / `os.environ.get("AZURE_OPENAI_...")` lookup, an argparse/CLI flag name, an error message referencing the variable name, or a test fixture using `monkeypatch.setenv(...)` with an obviously fake placeholder value (e.g. `https://example.openai.azure.com/openai/v1`, `https://endpoint.example.openai.azure.com/openai/v1`). No literal API key value was found assigned to `AZURE_OPENAI_API_KEY` or any sibling variable anywhere in history.
    - `127.0.0.1` occurs exactly 3 times in history, all as the default bind host for the local `sopkb serve` command (`--host` default and `serve_bundle(..., host: str = "127.0.0.1", ...)`), plus one doc reference to `http://127.0.0.1:8765` as the local URL to open. `localhost` does not occur at all.
    - This confirms, rather than merely assumes, the expected claim going into this pass: the repository's only "secret-shaped" strings are `AZURE_OPENAI_*` environment variable names (never values) and `127.0.0.1` used as a local default bind address — nothing else was found.

## Recommendation

!!! warning "A real secret-scanning tool should gate the transition to public"
    This manual sweep is a reasonable stopgap given the small history (7 commits) and the tooling available, but it should not be the long-term control. Before this repository is flipped to public, a real secret-scanning tool should run as a gating CI check — `gitleaks` is a good default choice (fast, no external service dependency, straightforward GitHub Action), though `trufflehog` or `ggshield` are acceptable alternatives.

    That CI workflow is being added separately on `feat/ci-cd-quality-gates` and is out of scope for this change; this document only records the manual results and the recommendation, not the workflow file itself.

## Out of scope: main/dev branch split

`main` currently contains only a `.gitkeep` and an unrelated `BRAINSTORM.md`, while the actual project lives on `dev` — this is a known, pre-existing split that has been explicitly deferred as a separate task by the repo owner and is not addressed by this pass.
