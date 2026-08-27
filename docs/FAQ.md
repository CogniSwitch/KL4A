---
title: FAQ
---

# FAQ / Troubleshooting

This page covers the desktop app specifically — for CLI-only usage, error text may
differ slightly since the CLI and the app resolve settings through the same
underlying config layer but surface errors differently.

## Getting started

### The bundle picker says "No bundles found in this workbench root." or "Could not load bundles"

The app only recognizes a **Workbench root** — a folder whose `knowledge-bundles/`
subdirectory contains one or more bundles (`<root>/knowledge-bundles/<your-bundle>/`).
Selecting a folder that has no `knowledge-bundles/` subdirectory populated with
bundles (including a bundle directory itself, or `knowledge-bundles/` directly
instead of its parent) produces the empty state — "No bundles found in this
workbench root." — not an error, since the scan itself succeeded and just found
nothing. **"Could not load bundles"** is a different, harder failure — the bundle
listing call itself failed — worth reporting as a bug if you hit it.

## Ingesting sources

### A `.docx` or `.pdf` source fails to ingest, or shows no extracted content

Both formats are supported by the desktop app. If ingestion fails outright, that's
worth reporting as a bug. If it succeeds but the source ends up with no usable
content, the source is marked with a parse failure and a warning rather than
aborting the whole ingest run — check the **Sources** screen for the source's
warning text, which will read one of:

- `PDF text extraction produced no content`
- `DOCX text extraction produced no content`

This means the file parsed, but no text could be pulled from it — the most common
cause is a scanned or image-only PDF (no OCR is performed) or an empty/corrupt
document. Markdown/plain-text sources never hit this path.

## Mining and LLM setup

### Ingest screen shows "No LLM provider configured yet — set one up in Settings to enable LLM-based mining."

The **Mining provider** dropdown on the Ingest screen defaults to `azure-llm` once
an LLM profile exists, but falls back to `fixture` — and shows this message instead
of the dropdown — when no usable profile is configured yet.

**The fastest fix is usually to avoid this entirely.** Leave the provider on
**fixture**: zero setup, no key, no network call. It extracts obligation-shaped
sentences with plain pattern matching. (Only `fixture` and `azure-llm` are valid
providers — the dropdown never offers a third option, so you can't hit an
"invalid provider" error from the UI; it only surfaces if you're scripting
directly against `sopkb-cli`.)

If you do want LLM-based mining: open **Settings** → **LLM profiles** → **+ New
profile**, and fill in **Name**, **Base URL**, and **Model** (all three are required
— the Save button stays disabled until they're non-blank), plus **Auth style**, **Max
output tokens**, **Timeout (seconds)**, **Reasoning effort**, and **API key** as
needed. Use **Test** on the saved profile to verify the connection before mining
against it.

One thing worth knowing about precedence: a field with an **"env override active"**
badge next to it (shown on Base URL and API key) means an environment variable is
currently overriding whatever you type there — editing it in the UI has no effect
until that environment variable is unset. Env vars always win over a saved profile
value.

## Review

### The Approve/Reject/Defer/Edit buttons are greyed out on a knowledge item

Once a knowledge item has been **approved** or **rejected** on the Review screen,
that status is terminal by design — the app disables those actions based on the
item's `allowed_actions` rather than letting you attempt one and fail. Only
**comment** stays available on a terminal item. This is intentional: it keeps a
reviewed item's history unambiguous. If you need to revise an approved/rejected
item, that's a deliberate re-ingest decision, not a review action — re-run mining
on the source to regenerate the underlying knowledge item. (The backend error
behind this, if you're scripting against `sopkb-cli` instead: `cannot change
terminal review status: approved`.)

## Settings and data

### Where are my saved settings and API key actually stored?

`~/.sopkb/settings.json` (override the location with the `SOPKB_SETTINGS_PATH`
environment variable — the Settings screen's intro text states the actual path in
effect). The API key is stored in plaintext there, not encrypted; the file is
chmod'd `0600` (owner read/write only) on macOS/Linux, but this tightening doesn't
happen on Windows — there's no equivalent step in the code for that platform.

---

Found a gap that's not on this page? Please
[open an issue](https://github.com/CogniSwitch/KL4A/issues/new/choose).
