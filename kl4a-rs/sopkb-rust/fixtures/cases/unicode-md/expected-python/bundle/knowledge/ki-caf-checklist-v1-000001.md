---
type: SOP Knowledge Piece
title: "\xC9ligibilit\xE9"
description: "Le clinicien doit confirmer l'identit\xE9 du patient."
resource: ../sections/caf-checklist/section-caf-checklist-002.md
tags:
- knowledge
- requires
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-caf-checklist
  title: "caf\xE9 checklist"
  resource: ../sources/caf-checklist.md
sopkb:
  knowledge_item_id: ki-caf-checklist-v1-000001
  source_id: caf-checklist
  source_version_id: caf-checklist:v1
  section_id: section-caf-checklist-002
  review_status: proposed
  lifecycle_status: active
  confidence: 0.82
  span_status: exact
  evidence: ../evidence/evidence-ki-caf-checklist-v1-000001.md
  knowledge_relation: ../relations/kr-ki-caf-checklist-v1-000001.md
  decision_rules:
  - ../rules/rule-ki-caf-checklist-v1-000001-requires.md
  structured_statement:
    subject: "\xC9ligibilit\xE9"
    predicate: requires
    object: "Le clinicien doit confirmer l'identit\xE9 du patient."
---
# Éligibilité

## Structured Statement

| Field | Value |
| --- | --- |
| Subject | [Éligibilité](../concepts/concept-ligibilit.md) |
| Predicate | `requires` |
| Object | Le clinicien doit confirmer l'identité du patient. |

## Evidence

- [evidence-ki-caf-checklist-v1-000001](../evidence/evidence-ki-caf-checklist-v1-000001.md)

## Relations

- [kr-ki-caf-checklist-v1-000001](../relations/kr-ki-caf-checklist-v1-000001.md)

## Decision Rules

- [Éligibilité](../rules/rule-ki-caf-checklist-v1-000001-requires.md)

## Source Context

Le clinicien doit confirmer l'identité du patient. [^src-caf-checklist]

[^src-caf-checklist]: café checklist section `section-caf-checklist-002`.
