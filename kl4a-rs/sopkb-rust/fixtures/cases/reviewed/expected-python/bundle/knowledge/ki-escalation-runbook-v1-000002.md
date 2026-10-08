---
type: SOP Knowledge Piece
title: Secondary Contact
description: Clinicians should route unresolved alerts to the secondary on-call physician.
resource: ../sections/escalation-runbook/section-escalation-runbook-003.md
tags:
- knowledge
- should
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
  knowledge_item_id: ki-escalation-runbook-v1-000002
  source_id: escalation-runbook
  source_version_id: escalation-runbook:v1
  section_id: section-escalation-runbook-003
  review_status: proposed
  lifecycle_status: active
  confidence: 0.82
  span_status: exact
  evidence: ../evidence/evidence-ki-escalation-runbook-v1-000002.md
  knowledge_relation: ../relations/kr-ki-escalation-runbook-v1-000002.md
  decision_rules:
  - ../rules/rule-ki-escalation-runbook-v1-000002-should.md
  structured_statement:
    subject: Secondary Contact
    predicate: should
    object: Clinicians should route unresolved alerts to the secondary on-call physician.
---
# Secondary Contact

## Structured Statement

| Field | Value |
| --- | --- |
| Subject | [Secondary Contact](../concepts/concept-secondary-contact.md) |
| Predicate | `should` |
| Object | Clinicians should route unresolved alerts to the secondary on-call physician. |

## Evidence

- [evidence-ki-escalation-runbook-v1-000002](../evidence/evidence-ki-escalation-runbook-v1-000002.md)

## Relations

- [kr-ki-escalation-runbook-v1-000002](../relations/kr-ki-escalation-runbook-v1-000002.md)

## Decision Rules

- [Secondary Contact](../rules/rule-ki-escalation-runbook-v1-000002-should.md)

## Source Context

Clinicians should route unresolved alerts to the secondary on-call physician. [^src-escalation-runbook]

[^src-escalation-runbook]: escalation runbook section `section-escalation-runbook-003`.
