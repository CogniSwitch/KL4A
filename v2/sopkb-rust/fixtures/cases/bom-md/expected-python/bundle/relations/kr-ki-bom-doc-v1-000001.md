---
type: SOP Knowledge Relation
title: kr-ki-bom-doc-v1-000001
description: Document requires Staff must confirm patient identity before proceeding..
resource: ../knowledge/ki-bom-doc-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-bom-doc
  title: bom doc
  resource: ../sources/bom-doc.md
sopkb:
  relation:
    id: kr-ki-bom-doc-v1-000001
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
      id: object-staff-must-confirm-patient-identity-before-proceeding
      text: Staff must confirm patient identity before proceeding.
      label: Staff must confirm patient identity before proceeding.
    knowledge_piece_id: ki-bom-doc-v1-000001
    evidence_id: evidence-ki-bom-doc-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-bom-doc-v1-000001

## Assertion

- Subject: [Document](../concepts/concept-document.md)
- Predicate: `requires`
- Object: Staff must confirm patient identity before proceeding.

## Connected Knowledge

- Knowledge piece: [ki-bom-doc-v1-000001](../knowledge/ki-bom-doc-v1-000001.md)
- Evidence: [evidence-ki-bom-doc-v1-000001](../evidence/evidence-ki-bom-doc-v1-000001.md)
- Decision rule: [Confirm patient identity](../rules/rule-ki-bom-doc-v1-000001-confirm-patient-identity.md)
