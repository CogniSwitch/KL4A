---
type: SOP Knowledge Relation
title: kr-ki-authored-okf-source-v1-000001
description: Requirements requires Staff must confirm the manual override policy before
  deviating.
resource: ../knowledge/ki-authored-okf-source-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-authored-okf-source
  title: authored okf source
  resource: ../sources/authored-okf-source.md
sopkb:
  relation:
    id: kr-ki-authored-okf-source-v1-000001
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
      id: object-staff-must-confirm-the-manual-override-policy-before-deviating
      text: Staff must confirm the manual override policy before deviating from the
        standard SOP.
      label: Staff must confirm the manual override policy before deviating
    knowledge_piece_id: ki-authored-okf-source-v1-000001
    evidence_id: evidence-ki-authored-okf-source-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-authored-okf-source-v1-000001

## Assertion

- Subject: [Requirements](../concepts/concept-requirements.md)
- Predicate: `requires`
- Object: Staff must confirm the manual override policy before deviating from the standard SOP.

## Connected Knowledge

- Knowledge piece: [ki-authored-okf-source-v1-000001](../knowledge/ki-authored-okf-source-v1-000001.md)
- Evidence: [evidence-ki-authored-okf-source-v1-000001](../evidence/evidence-ki-authored-okf-source-v1-000001.md)
- Decision rule: [Requirements](../rules/rule-ki-authored-okf-source-v1-000001-requires.md)
