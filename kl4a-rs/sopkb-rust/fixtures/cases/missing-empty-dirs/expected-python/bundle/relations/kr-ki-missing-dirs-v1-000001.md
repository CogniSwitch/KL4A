---
type: SOP Knowledge Relation
title: kr-ki-missing-dirs-v1-000001
description: Requirements requires Staff must confirm the checklist before closing
  the case..
resource: ../knowledge/ki-missing-dirs-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-missing-dirs
  title: missing dirs
  resource: ../sources/missing-dirs.md
sopkb:
  relation:
    id: kr-ki-missing-dirs-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-requirements
      label: Requirements
      text: Requirements
      okf_path: concepts/concept-requirements.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-staff-must-confirm-the-checklist-before-closing-the-case
      text: Staff must confirm the checklist before closing the case.
      label: Staff must confirm the checklist before closing the case.
    knowledge_piece_id: ki-missing-dirs-v1-000001
    evidence_id: evidence-ki-missing-dirs-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-missing-dirs-v1-000001

## Assertion

- Subject: [Requirements](../concepts/concept-requirements.md)
- Predicate: `requires`
- Object: Staff must confirm the checklist before closing the case.

## Connected Knowledge

- Knowledge piece: [ki-missing-dirs-v1-000001](../knowledge/ki-missing-dirs-v1-000001.md)
- Evidence: [evidence-ki-missing-dirs-v1-000001](../evidence/evidence-ki-missing-dirs-v1-000001.md)
- Decision rule: [Requirements](../rules/rule-ki-missing-dirs-v1-000001-requires.md)
