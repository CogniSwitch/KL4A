---
type: SOP Knowledge Relation
title: kr-ki-escalation-runbook-v1-000003
description: Vendor Escalation records The vendor liaison must record every escalation
  in the.
resource: ../knowledge/ki-escalation-runbook-v1-000003.md
tags:
- relation
- rdf-compatible
- records
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-escalation-runbook
  title: escalation runbook
  resource: ../sources/escalation-runbook.md
sopkb:
  relation:
    id: kr-ki-escalation-runbook-v1-000003
    type: Knowledge Relation
    subject:
      id: concept-vendor-escalation
      label: Vendor Escalation
      text: Vendor Escalation
      okf_path: concepts/concept-vendor-escalation.md
    predicate:
      id: predicate-records
      text: records
    object:
      id: object-the-vendor-liaison-must-record-every-escalation-in-the
      text: The vendor liaison must record every escalation in the incident log.
      label: The vendor liaison must record every escalation in the
    knowledge_piece_id: ki-escalation-runbook-v1-000003
    evidence_id: evidence-ki-escalation-runbook-v1-000003
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-escalation-runbook-v1-000003

## Assertion

- Subject: [Vendor Escalation](../concepts/concept-vendor-escalation.md)
- Predicate: `records`
- Object: The vendor liaison must record every escalation in the incident log.

## Connected Knowledge

- Knowledge piece: [ki-escalation-runbook-v1-000003](../knowledge/ki-escalation-runbook-v1-000003.md)
- Evidence: [evidence-ki-escalation-runbook-v1-000003](../evidence/evidence-ki-escalation-runbook-v1-000003.md)
- Decision rule: [Vendor Escalation](../rules/rule-ki-escalation-runbook-v1-000003-records.md)
