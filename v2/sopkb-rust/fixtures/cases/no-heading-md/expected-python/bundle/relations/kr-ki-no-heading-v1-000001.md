---
type: SOP Knowledge Relation
title: kr-ki-no-heading-v1-000001
description: Document requires Staff must confirm patient identity before dispensing
  medication..
resource: ../knowledge/ki-no-heading-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-no-heading
  title: no heading
  resource: ../sources/no-heading.md
sopkb:
  relation:
    id: kr-ki-no-heading-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-document
      label: Document
      text: Document
      okf_path: concepts/concept-document.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-staff-must-confirm-patient-identity-before-dispensing-medication
      text: Staff must confirm patient identity before dispensing medication.
      label: Staff must confirm patient identity before dispensing medication.
    knowledge_piece_id: ki-no-heading-v1-000001
    evidence_id: evidence-ki-no-heading-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-no-heading-v1-000001

## Assertion

- Subject: [Document](../concepts/concept-document.md)
- Predicate: `requires`
- Object: Staff must confirm patient identity before dispensing medication.

## Connected Knowledge

- Knowledge piece: [ki-no-heading-v1-000001](../knowledge/ki-no-heading-v1-000001.md)
- Evidence: [evidence-ki-no-heading-v1-000001](../evidence/evidence-ki-no-heading-v1-000001.md)
- Decision rule: [Confirm patient identity](../rules/rule-ki-no-heading-v1-000001-confirm-patient-identity.md)
