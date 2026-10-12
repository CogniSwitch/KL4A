---
title: SOPKB Demo — CISA Incident Response
description: Evaluate evidence, review, and grounded agent use against a real public incident-response playbook.
---

# SOPKB Demo: CISA Incident-Response Playbook

This demo turns a real operational playbook into a reviewed SOP Knowledge
Bundle. It demonstrates the distinction that matters: an agent can answer from
approved, source-linked material, and must say when the bundle does not cover a
question. The playbook is a public reference, **not** advice or a substitute
for an organization's incident-response policy.

## Fixed input

Use only this edition for the demo:

- Source: [Federal Government Cybersecurity Incident and Vulnerability Response Playbooks (CISA, November 2021)](https://www.cisa.gov/sites/default/files/publications/Federal_Government_Cybersecurity_Incident_and_Vulnerability_Response_Playbooks_508C.pdf)
- Expected SHA-256: `2277247542D844D2258C0F6892D649D6885F783FC629664924E60638086F47F0`
- Expected size: `2703130` bytes

Download it and verify it before ingestion. On PowerShell:

```powershell
New-Item -ItemType Directory -Force demo-sop\sources | Out-Null
Invoke-WebRequest `
  -Uri 'https://www.cisa.gov/sites/default/files/publications/Federal_Government_Cybersecurity_Incident_and_Vulnerability_Response_Playbooks_508C.pdf' `
  -OutFile demo-sop\sources\cisa-incident-response-playbook.pdf
(Get-FileHash demo-sop\sources\cisa-incident-response-playbook.pdf -Algorithm SHA256).Hash
(Get-Item demo-sop\sources\cisa-incident-response-playbook.pdf).Length
```

Stop if either result differs. Record the download date; do not silently use a
later document revision.

## Prerequisites

Download the current Windows `kl4a` release asset and use its real filename:

```powershell
$kl4a = (Resolve-Path .\kl4a-windows-x86_64.exe)
& $kl4a --version
```

On macOS or Linux, use the corresponding `kl4a-macos-universal` or
`kl4a-linux-x86_64` asset, make it executable (`chmod +x`), and substitute its
path below. Configure an approved real mining provider before the `mine` step;
do **not** use a fixture provider for this real-input evaluation.

## Run

```powershell
& $kl4a --use sopkb init demo-sop\bundle
& $kl4a --use sopkb scan demo-sop\sources --bundle demo-sop\bundle
& $kl4a --use sopkb normalize demo-sop\bundle
& $kl4a --use sopkb mine demo-sop\bundle --provider azure-llm
& $kl4a --use sopkb validate demo-sop\bundle
```

If the selected provider is not `azure-llm`, substitute the provider that is
configured for the evaluator's environment and record its identifier. Do not
replace a provider failure with manually authored claims.

## Human review and agent evidence

Review a bounded set of proposed items related to containment, eradication, or
recovery. Approve only items whose displayed evidence exactly supports the
claim. Reject or edit at least one item if its wording overreaches its evidence;
record the rationale. This is a review demonstration, not a quota to approve
claims.

Then retrieve context:

```powershell
& $kl4a --use sopkb agent context demo-sop\bundle --task incident-response
```

Connect an MCP-capable agent with:

```json
{
  "mcpServers": {
    "sopkb-demo": {
      "command": "C:\\absolute\\path\\to\\sopkb-mcp-windows-x86_64.exe",
      "args": ["C:\\absolute\\path\\to\\demo-sop\\bundle"]
    }
  }
}
```

Ask: “According to this playbook, what should the response team do during
containment, eradication, and recovery after a confirmed ransomware incident?”
The agent must call the bundle tools first and cite returned evidence IDs or
source excerpts. Then ask: “What is our customer refund window?” The correct
outcome is an explicit statement that the bundle has no grounded answer.

## Evaluator checklist

- [ ] Input hash and file size match the fixed input above.
- [ ] `validate` completes with no errors; warnings are retained in the record.
- [ ] At least three answer statements can be traced to returned evidence.
- [ ] A reviewer recorded an approval and either an edit, rejection, or a reason
      why every inspected item was left proposed.
- [ ] The agent did not add operational claims absent from the bundle.
- [ ] The unrelated refund question produced no fabricated answer.

## Limitations

- This public federal playbook does not define the evaluator's own policy.
- Mining quality depends on the configured provider; review is required before
  claims become usable knowledge.
- The evaluation measures grounding and review behavior, not whether an
  incident-response plan is complete or compliant.
