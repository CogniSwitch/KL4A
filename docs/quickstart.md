---
title: Quickstart
---

# Quickstart

KL4A is a native desktop app. Install it, open it, and everything else — creating a
bundle, ingesting sources, reviewing mined knowledge, exporting, talking to the agent —
happens inside the app itself. There's no CLI you need to touch to get started.

## Get the app

| Platform | Download |
|---|---|
| **Windows** | [⬇ Installer (`.exe`)](https://github.com/CogniSwitch/KL4A/releases/download/0.0.1-alpha/KL4A.Workbench_0.0.1-alpha_x64-setup.exe) |
| **macOS** | [⬇ Disk image (`.dmg`)](https://github.com/CogniSwitch/KL4A/releases/download/0.0.1-alpha/KL4A.Workbench_0.0.1-alpha_universal.dmg) — universal, runs on Apple Silicon and Intel |
| **Linux** | [⬇ AppImage](https://github.com/CogniSwitch/KL4A/releases/download/0.0.1-alpha/KL4A.Workbench_0.0.1-alpha_amd64.AppImage) |

!!! warning "The builds aren't code-signed yet"
    No Apple Developer ID or Windows signing certificate is set up for this project
    yet, so your OS will warn you on first launch:

    - **macOS** — Gatekeeper blocks it as "from an unidentified developer". Right-click
      the app in Finder and choose **Open** (once), or run `xattr -cr` on it.
    - **Windows** — SmartScreen shows "Windows protected your PC". Click **More info**
      → **Run anyway**.
    - **Linux** — mark the AppImage executable first: `chmod +x KL4A*.AppImage`.

## First launch

On first launch you land on the bundle picker. Create a new bundle, point it at a
folder of SOPs/policies/procedures (Markdown, DOCX, or PDF), and the app walks you
through the rest: ingest → inspect what got mined and why → review (approve, reject,
defer, edit, comment) → export.

See the [Desktop UI Guide](DESKTOP_UI_GUIDE.md) for a full screen-by-screen tour.

## A richer example

Once you're comfortable with the app, open the included **GLP-1 Healthcare
SOP** reference bundle — a finished, evidence-linked bundle with persisted review
states, conflict/freshness reporting, and full OKF/graph/RDF export:

→ **[examples/glp1-healthcare](https://github.com/CogniSwitch/KL4A/tree/main/examples/glp1-healthcare)**

## Next steps

- [Desktop UI Guide](DESKTOP_UI_GUIDE.md) — the desktop app, screen by screen.
- [MCP Server](MCP_SERVER.md) — connecting an agent to a bundle you've built.
- [FAQ.md](FAQ.md) — common setup and ingestion problems.
- [Contributing](CONTRIBUTING.md) — if you want to work on KL4A itself.
