---
type: SOP Knowledge Relation
title: kr-ki-item-v1-000003
description: "\u6982\u8FF0 requires Staff must confirm patient identity.."
resource: ../knowledge/ki-item-v1-000003.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-item
  title: "\u5317\u4EAC"
  resource: ../sources/item.md
sopkb:
  relation:
    id: kr-ki-item-v1-000003
    type: Knowledge Relation
    subject:
      id: concept-item
      label: "\u6982\u8FF0"
      text: "\u6982\u8FF0"
      okf_path: concepts/concept-item.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-staff-must-confirm-patient-identity
      text: Staff must confirm patient identity.
      label: Staff must confirm patient identity.
    knowledge_piece_id: ki-item-v1-000003
    evidence_id: evidence-ki-item-v1-000003
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-item-v1-000003

## Assertion

- Subject: [概述](../concepts/concept-item.md)
- Predicate: `requires`
- Object: Staff must confirm patient identity.

## Connected Knowledge

- Knowledge piece: [ki-item-v1-000003](../knowledge/ki-item-v1-000003.md)
- Evidence: [evidence-ki-item-v1-000003](../evidence/evidence-ki-item-v1-000003.md)
- Decision rule: [Confirm patient identity](../rules/rule-ki-item-v1-000003-confirm-patient-identity.md)
