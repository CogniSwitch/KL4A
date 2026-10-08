---
title: "Launch Checklist: SOP Knowledge Workbench Going Public"
---

# Launch Checklist: SOP Knowledge Workbench Going Public

Date: 2026-08-07

## Purpose and how to read this

This is a **status report, not a launch announcement**. Ten `feat/*` branches
were built independently off `dev` to prepare this repository for a public
OSS release. None of them have been merged into `dev` or `main`. None of
them, and none of `dev` itself, have been pushed to `origin`. This document
was produced by directly inspecting each branch's real commits and file
content (`git log`, `git show`, `git diff --stat` against `dev`) — not by
copying an earlier plan and marking boxes from memory.

Status markers used below:

- ✅ **Drafted** (branch `feat/x`, commit `<hash>`) — the content exists and
  was verified present on that branch, but is **not yet merged** into `dev`
  or `main`, and nothing has been pushed to `origin`.
- ⚠️ **Needs a human decision/action** — real content exists, but something
  beyond a code review is required: a placeholder needs a real value, a
  one-time manual step on an external site (PyPI, GitHub) is required, or a
  policy/legal confirmation is outstanding.
- ❌ **Not started** — no branch addresses this; it remains open work.

Every hash below is a real, verified commit reachable from the named branch
(all obtained via `git log <branch> --oneline` and `git rev-parse --short
<branch>`), and every "drafted" claim was checked with `git show
<branch>:<path>` or `git diff dev..<branch>`.

---

## 1. `main` contains the actual project

!!! danger "Not started"

`main` currently contains a single file, `README.md`, with one line of
content (`# Knowledge-Workbench`, no trailing newline) — commit `1a3af07`.
It does not contain any of the `tools/sopkb` code, docs, or examples that
live on `dev` (commit `a19d1ee`, "Add SOP knowledge workbench files").

Note on the assumed starting state: the working assumption going into this
audit was that `main` held a `.gitkeep` plus a `BRAINSTORM.md`. That is not
what's actually there — there is no `BRAINSTORM.md` and no `.gitkeep`
anywhere in this repository's history (`git log --all --diff-filter=A
--name-only` finds neither, on any ref). The practical conclusion is the
same either way: **`main` does not contain the project**. Promoting
`dev` (or a merge of it plus these ten branches) to `main` is explicitly a
separate, deferred piece of work per the repo owner — it is intentionally
out of scope for this checklist and for the branch work it describes.

---

## 2. Legal: license, copyright, third-party notices

!!! success "Drafted — branch `feat/legal-licensing`, commit `a9f3c05`"

- `LICENSE` — full Apache License 2.0 text, 202 lines.
- `NOTICE` — copyright line `Copyright 2026 CogniSwitch`; a trademark
  notice separating the OSS project name from the "CogniSwitch" /
  "CS Governed KB" proprietary product names; and a third-party notice
  auditing the three runtime dependencies (PyYAML, pdfplumber, python-docx —
  all MIT, all compatible with Apache-2.0).

!!! warning "Needs a human decision"
    The copyright holder in `NOTICE` ("CogniSwitch") is asserted by the
    branch author, not confirmed by legal/founders. Per the original plan
    this needs an explicit sign-off before the license takes effect
    publicly — it's a one-line change if approved, but it hasn't been
    reviewed by anyone with the authority to confirm it.

---

## 3. Secret / history scan

!!! warning "Manual scan done and clean; automated CI scan never implemented as a running job"

Branch `feat/security-hygiene`, commit `7b40954` ("Add manual secret scan
notes ahead of going public") adds `docs/SECURITY_SCAN_NOTES.md`. It records
a **manual** sweep (`git log --all -p | grep`/ripgrep, since `gitleaks`,
`trufflehog`, and `ggshield` were all unavailable in that environment)
across all 7 commits reachable from any local ref at the time, plus the
current working tree.

Findings — none of the following anywhere in history:

- AWS keys
- private-key headers
- generic password/secret/token literals
- credential-bearing connection strings
- vendor token prefixes
- committed `.env`/key files

`AZURE_OPENAI_*` occurs only as environment-variable names, never values;
`127.0.0.1` occurs only as the local `sopkb serve` bind address.

The document itself explicitly recommends that a real scanner (it suggests
`gitleaks`) run as a **gating CI check** before the repo goes public, and
says that workflow is "being added separately on `feat/ci-cd-quality-gates`."
That branch was checked directly: **it is not there.**
`.github/workflows/ci.yml`, `lint.yml`, and `codeql.yml` on
`feat/ci-cd-quality-gates` (commit `8257e7d`) contain no `gitleaks`,
`trufflehog`, or any other secret-scanning step, and neither does
`.pre-commit-config.yaml` (ruff lint/format only).

**So: the manual pass is
real and clean, but there is currently no automated secret scan anywhere in
the ten branches, and (since nothing has been pushed) no scan has ever
actually run against the pushed repository.** This is a real gap, not just
an unexecuted-but-ready job.

---

## 4. `docs/DISCUSSION_CAPTURE.md` rewritten for a public audience

!!! success "Drafted — branch `feat/security-hygiene`, commit `d3f8af0`"

The original internal file (93 lines, referencing internal product names
like "CSKS"/"NSCS" and internal roadmap framing) is replaced. `dev`'s current
`docs/DISCUSSION_CAPTURE.md` is the pre-rewrite internal version; the
public-safe version only exists on `feat/security-hygiene`.

Same branch also strengthens the synthetic-data disclaimer in
`examples/glp1-healthcare/README.md` (commit `29a3f4d`) — the one-line
"this example is synthetic" note becomes an explicit blockquote disclaimer
covering fabricated data, no real PII/PHI, and "not clinical/legal/regulatory
guidance." It also adds `.github/dependabot.yml` for `pip` (scoped to
`tools/sopkb`) and `github-actions` update tracking (commit `cfd7e1b`, the
branch tip).

Also on this branch: `docs/WHY_OPEN_CORE.md` (commit not separately
isolated from the above — present at the branch tip), explaining the
OSS/enterprise boundary for contributors and integrators.

---

## 5. Root community/process docs in place

!!! success "Drafted — branch `feat/docs-set`, single commit `d1c32d3`"

Verified present on that branch:

- `README.md` (rewritten from the one-line stub)
- `QUICKSTART.md`
- `CONTRIBUTING.md`
- `CODE_OF_CONDUCT.md`
- `SECURITY.md`
- `SUPPORT.md`
- `docs/CLI_REFERENCE.md`
- `docs/WEB_UI_GUIDE.md`
- `docs/ARCHITECTURE.md`
- `docs/README.md`
- `FAQ.md`

- **CHANGELOG.md**, **GOVERNANCE.md**, and **ROADMAP.md** are *not* on this
  branch — they're drafted separately (see §9 and §10 below).
- **FAQ/Troubleshooting**: the file used is `FAQ.md` (titled "FAQ /
  Troubleshooting" inside), not a separately named `TROUBLESHOOTING.md`.
- **CONTRIBUTING.md** documents DCO sign-off (`git commit -s`) as the
  contribution-licensing mechanism (no separate CLA).

---

## 6. README quickstart + fixture-provider path

!!! success "Drafted — branch `feat/docs-set`, commit `d1c32d3`"

The rewritten root `README.md` includes a "Quickstart (no API key, no
network)" section that walks through `pip install sopkb` →
`init`/`scan`/`normalize`/`mine --provider fixture`/`validate`/`export`/
`serve`, explicitly calling out the `fixture` mining provider as needing "no
LLM, no API key, no network call," with a pointer to `azure-llm` as the
optional richer-extraction path (documented further in `FAQ.md`).

---

## 7. Screenshot / GIF in README

!!! warning "Left as a placeholder — no image was actually added"
    Checked directly: the README diff on `feat/docs-set` inserts an HTML
    comment, not an image:

    ```html
    <!--
      TODO: capture a real screenshot or short GIF of the web UI (sopkb serve) —
      the Review or Agent screen makes a good hero shot — and drop it in here,
      e.g. as docs/images/web-ui.png. Do not fabricate a placeholder image.
    -->
    ```

    No `docs/images/` directory or image file was added on this branch. This
    is honest and intentional (the comment explicitly says not to fabricate a
    placeholder image), but it means **there is currently no visual in the
    README** — a real screenshot/GIF still needs to be captured against a
    real `sopkb serve` run before launch.

---

## 8. `docs/WEB_UI_GUIDE.md` and `docs/CLI_REFERENCE.md`

!!! success "Drafted — branch `feat/docs-set`, commit `d1c32d3`"

- `docs/CLI_REFERENCE.md` — 215 lines, documents every `sopkb` subcommand
  and flag.
- `docs/WEB_UI_GUIDE.md` — 113 lines, covers `sopkb serve` and the
  sources/ingest/review/agent-chat/reports/graph/export surfaces.

---

## 9. Root community docs — CHANGELOG, GOVERNANCE, ROADMAP

!!! success "Drafted — on two further branches"

- **GOVERNANCE.md** — branch `feat/community-governance`, commit `78e2ca3`
  ("Add GOVERNANCE.md: OSS/enterprise boundary, maintainer roles, decision
  process"). Documents maintainer roles, decision process, and explicitly
  states "CogniSwitch engineers are the de facto maintainers" for now — no
  named individual triage owner (ties to §14 below).
- **CHANGELOG.md** and **ROADMAP.md** — branch `feat/roadmap`, commit
  `2667334` ("Add CHANGELOG.md and ROADMAP.md"). `CHANGELOG.md`'s only
  version heading is `## [0.0.2] - TBD (unreleased)` — consistent with no
  tag having been cut yet (§16).

---

## 10. `.github` templates and CODEOWNERS

!!! success "Drafted — branch `feat/github-repo-config`, commit `e904871`"

Verified present: `.github/ISSUE_TEMPLATE/bug_report.md`,
`feature_request.md`, `extraction_quality.md`, `.github/ISSUE_TEMPLATE/
config.yml` (blank issues disabled, routes to Discussions/Security
Advisories instead), `.github/PULL_REQUEST_TEMPLATE.md`, and
`.github/CODEOWNERS`.

!!! warning "Needs a human decision"
    `CODEOWNERS` uses `@CogniSwitch/maintainers` for every path, and the
    file's own header comment flags this — "Placeholder handles/teams below
    (@CogniSwitch/*) need to be replaced with real GitHub usernames or teams
    before this takes effect — GitHub silently ignores CODEOWNERS entries
    that reference teams/users without write access to the repo." Nobody has
    confirmed that `@CogniSwitch/maintainers` exists as a real GitHub team
    with write access.

---

## 11. CI green on a clean clone

!!! danger "Not true yet — and cannot currently be true"
    Workflows are drafted; none have ever run.

Branch `feat/ci-cd-quality-gates`, commit `8257e7d` ("Add CI/CD quality-gate
workflows and pre-commit config") adds `.github/workflows/ci.yml`,
`lint.yml`, `codeql.yml`, `release.yml`, and `.pre-commit-config.yaml`.
These are real, reasonably complete workflow definitions:

- `ci.yml` runs pytest across ubuntu/windows × Python 3.11–3.13
- `lint.yml` runs ruff
- `codeql.yml` runs CodeQL analysis
- `release.yml` builds and publishes on a `v*` tag push via PyPI trusted publishing

**But since nothing has been
pushed to `origin` on any branch, none of these workflows have ever
executed.** "CI is green" is unverified — it is not the same claim as "CI
has never failed."

There is also a real cross-branch ordering dependency worth flagging
explicitly: `ci.yml`'s own header comment says it assumes
`tools/sopkb/pyproject.toml` already has the `[project.optional-dependencies]
dev = [...]` extra that is actually added on `feat/packaging-distribution`
(§12) — i.e. `ci.yml` was authored assuming `feat/packaging-distribution`
lands first. Until both are merged together, `pip install -e ".[dev]"` in
the CI workflow will fail.

---

## 12. `pyproject.toml` publish metadata; package name availability

!!! success "Drafted — branch `feat/packaging-distribution`, commit `478a3c2` — and package name independently confirmed available on PyPI"

`tools/sopkb/pyproject.toml` gains:

- `readme`
- `license = "Apache-2.0"` + `license-files`
- `authors`
- `keywords`
- `classifiers`
- `[project.optional-dependencies] dev`
- `[project.urls]` (Homepage/Repository/Issues/Documentation — the last pointing at `dev`
  until the `main`/`dev` split lands)

`tools/sopkb/LICENSE` (a synced copy of
the root LICENSE) and `tools/sopkb/Makefile` (install/test/lint/format/
build/serve-example/clean targets) are also added.

PyPI was independently queried directly (`curl https://pypi.org/pypi/sopkb/
json`) rather than relying on the branch's own claims: it returns HTTP 404 /
`{"message": "Not Found"}`, confirming **no package named `sopkb` currently
exists on PyPI** — the name is genuinely available, not just assumed to be.

!!! warning "Needs a human decision"
    The `authors` email is a deliberate placeholder —
    `oss-placeholder@example.invalid` — with a code comment flagging it
    needs to be replaced with a real, monitored address before publishing to
    PyPI. Also, per `release.yml`'s own header comment, PyPI **trusted
    publishing** requires a one-time manual setup on pypi.org (Settings →
    Publishing, registering the GitHub repo/workflow/environment) that only
    the PyPI project owner can do — that has not happened, and can't happen
    until a project named `sopkb` is first created on PyPI (the 404 above
    cuts both ways: the name is free, but there is also no project there yet
    to configure).

---

## 13. Branch protection + Security Advisories/Discussions enabled

!!! danger "Not started"

These are GitHub repo settings (UI or API), not files in
this git history, and cannot be flipped on a repo that hasn't been pushed
yet. `SECURITY.md` (drafted, §5) already assumes GitHub Security Advisories
as the reporting path, and `SUPPORT.md` (also drafted) already links to
GitHub Discussions — but neither feature has actually been enabled on the
real GitHub repo, and branch protection rules for `main`/`dev` have not been
configured at all.

---

## 14. v0.0.2 release process, notes, and tag

!!! success "Drafted — branch `feat/release-process`, commit `fe55bf6`"

- `docs/RELEASE_PROCESS.md` — SemVer policy, `vX.Y.Z` tag convention, what
  pushing a tag triggers (build → PyPI trusted-publish → GitHub Release
  notes sourced from `CHANGELOG.md`), pre-tag checklist, post-release
  verification.
- `docs/BUNDLE_COMPATIBILITY_POLICY.md` — bundle-format compatibility
  tracked separately from software SemVer.
- `docs/releases/v0.0.2.md` — user-facing release notes: bundle lifecycle,
  HITL review, local web workbench, the GLP-1 reference bundle, the MCP
  server, OKF export.

!!! danger "The `v0.0.2` git tag itself has not been created"
    Consistent with `CHANGELOG.md`'s `## [0.0.2] - TBD (unreleased)` heading
    (§9) — the release notes and process are ready, but no one has run
    `git tag -a v0.0.2` or pushed it, and per §12/§13 the PyPI/GitHub-side
    prerequisites for that tag to actually publish anything aren't in place
    yet either.

---

## 15. Post-launch operations playbook

!!! success "Drafted — branch `feat/post-launch-ops`, commit `65f7cc5`"

Adds `docs/POST_LAUNCH_OPERATIONS.md` (42 lines).

---

## 16. First-line triage owner named

!!! danger "Not started — placeholder only"

`SECURITY.md` (drafted on
`feat/docs-set`, §5) is explicit about this rather than glossing over it:

> **Triage owner:** TODO — name a real, monitored person or team here before
> this repository goes public. Until this is filled in, treat GitHub
> Security Advisories as the authoritative reporting path...

`GOVERNANCE.md` (§9) similarly only says "CogniSwitch engineers are the de
facto maintainers" for now, without naming an individual. No branch names a
real person or team as first-line triage owner.

---

## Summary tally

Across the ten `feat/*` branches inspected, using the checklist items in
this document as the unit of count:

| Status | Count | Items |
|---|---|---|
| ✅ Drafted | 10 of 10 branches | legal-licensing, security-hygiene, docs-set, packaging-distribution, ci-cd-quality-gates, github-repo-config, release-process, community-governance, roadmap, post-launch-ops — real, verified content matching expected scope. None merged into `dev`/`main`; none pushed to `origin`. |
| ⚠️ Needs a human decision/action | 8 items | §2 copyright holder confirmation, §3 automated secret-scan CI job never implemented/run, §7 missing README screenshot, §10 placeholder CODEOWNERS handles, §11 CI never actually executed, §12 placeholder PyPI author email + one-time trusted-publisher registration, §13 branch protection/Security Advisories/Discussions toggles, §14 `v0.0.2` tag not yet cut |
| ❌ Not started | 3 items | §1 `main`/`dev` project split (explicitly deferred by the repo owner, out of scope for this branch effort), §13 GitHub-side repo settings, §16 naming a real first-line triage owner |

---

## Integration path

All of the "drafted" work above exists only as **ten independent, unmerged
branches cut from `dev`**. None have been merged into each other, into
`dev`, or into `main`; none have been pushed to `origin`. Getting from here
to an actual public launch requires, roughly in this order:

1. **Review and merge each `feat/*` branch** into `dev` (or directly into
   whatever becomes the new `main`), resolving ordinary merge conflicts.
   Note the one known ordering dependency found during this audit:
   `feat/ci-cd-quality-gates`'s `ci.yml` assumes `feat/packaging-distribution`
   has already landed (§11) — merge packaging-distribution first, or fix up
   `ci.yml` in the merge.
2. **Resolve the open human decisions** listed under the ⚠️ items above:
   - legal confirms the copyright holder
   - someone replaces the placeholder PyPI contact email and CODEOWNERS handles
   - someone is named as first-line security triage owner
   - a real screenshot/GIF is captured
3. **Add the missing automated secret-scan CI job** (e.g. `gitleaks`) rather
   than relying on the one-time manual pass — this was explicitly called out
   in `docs/SECURITY_SCAN_NOTES.md` as a pre-public gating requirement and
   is currently not implemented anywhere.
4. **Do the main/dev branch split** — separately from this effort, per the
   repo owner's own stated plan (§1).
5. **Push to `origin`**, then actually flip the GitHub-side settings that
   can't be done from git alone: branch protection, Security Advisories,
   Discussions, and PyPI trusted-publisher registration for a newly-created
   `sopkb` project (§12/§13).
6. Only once steps 1–5 are done does it make sense to **cut and push the
   `v0.0.2` tag** (§14), which is the actual trigger for the release
   pipeline drafted in `feat/release-process`.

This document is a snapshot of real, verified branch content as of
2026-08-07. It is not evidence that any of the above has shipped.
