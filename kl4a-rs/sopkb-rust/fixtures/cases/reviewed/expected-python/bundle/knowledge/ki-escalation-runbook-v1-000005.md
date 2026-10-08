---
type: SOP Knowledge Piece
title: Patient Communication
description: Care coordinators must confirm the patient has been informed of the outcome.
resource: ../sections/escalation-runbook/section-escalation-runbook-006.md
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
  knowledge_item_id: ki-escalation-runbook-v1-000005
  source_id: escalation-runbook
  source_version_id: escalation-runbook:v1
  section_id: section-escalation-runbook-006
  review_status: proposed
  lifecycle_status: active
  confidence: 0.82
  span_status: exact
  evidence: ../evidence/evidence-ki-escalation-runbook-v1-000005.md
  knowledge_relation: ../relations/kr-ki-escalation-runbook-v1-000005.md
  decision_rules:
  - ../rules/rule-ki-escalation-runbook-v1-000005-requires.md
  structured_statement:
    subject: Patient Communication
    predicate: requires
    object: Care coordinators must confirm the patient has been informed of the outcome.
---
# Patient Communication

## Structured Statement

| Field | Value |
| --- | --- |
| Subject | [Patient Communication](../concepts/concept-patient-communication.md) |
| Predicate | `requires` |
| Object | Care coordinators must confirm the patient has been informed of the outcome. |

## Evidence

- [evidence-ki-escalation-runbook-v1-000005](../evidence/evidence-ki-escalation-runbook-v1-000005.md)

## Relations

- [kr-ki-escalation-runbook-v1-000005](../relations/kr-ki-escalation-runbook-v1-000005.md)

## Decision Rules

- [Patient Communication](../rules/rule-ki-escalation-runbook-v1-000005-requires.md)

## Source Context

Care coordinators must confirm the patient has been informed of the outcome. [^src-escalation-runbook]

[^src-escalation-runbook]: escalation runbook section `section-escalation-runbook-006`.
