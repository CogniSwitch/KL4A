---
type: SOP Knowledge Piece
title: Initial Notification
description: On-call staff must confirm receipt of the alert within 15 minutes.
resource: ../sections/escalation-runbook/section-escalation-runbook-002.md
tags:
- knowledge
- requires
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-escalation-runbook
  title: escalation runbook
  resource: ../sources/escalation-runbook.md
sopkb:
  knowledge_item_id: ki-escalation-runbook-v1-000001
  source_id: escalation-runbook
  source_version_id: escalation-runbook:v1
  section_id: section-escalation-runbook-002
  review_status: proposed
  lifecycle_status: active
  confidence: 0.82
  span_status: exact
  evidence: ../evidence/evidence-ki-escalation-runbook-v1-000001.md
  knowledge_relation: ../relations/kr-ki-escalation-runbook-v1-000001.md
  decision_rules:
  - ../rules/rule-ki-escalation-runbook-v1-000001-requires.md
  structured_statement:
    subject: Initial Notification
    predicate: requires
    object: On-call staff must confirm receipt of the alert within 15 minutes.
---
# Initial Notification

## Structured Statement

| Field | Value |
| --- | --- |
| Subject | [Initial Notification](../concepts/concept-initial-notification.md) |
| Predicate | `requires` |
| Object | On-call staff must confirm receipt of the alert within 15 minutes. |

## Evidence

- [evidence-ki-escalation-runbook-v1-000001](../evidence/evidence-ki-escalation-runbook-v1-000001.md)

## Relations

- [kr-ki-escalation-runbook-v1-000001](../relations/kr-ki-escalation-runbook-v1-000001.md)

## Decision Rules

- [Initial Notification](../rules/rule-ki-escalation-runbook-v1-000001-requires.md)

## Source Context

On-call staff must confirm receipt of the alert within 15 minutes. [^src-escalation-runbook]

[^src-escalation-runbook]: escalation runbook section `section-escalation-runbook-002`.
