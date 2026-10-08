---
type: SOP Knowledge Relation
title: kr-ki-policy-v1-000001
description: Controls requires Staff must confirm badge access before entering the
  restricted.
resource: ../knowledge/ki-policy-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-policy
  title: policy
  resource: ../sources/policy.md
sopkb:
  relation:
    id: kr-ki-policy-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-controls
      label: Controls
      text: Controls
      okf_path: concepts/concept-controls.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-staff-must-confirm-badge-access-before-entering-the-restricted
      text: Staff must confirm badge access before entering the restricted area.
      label: Staff must confirm badge access before entering the restricted
    knowledge_piece_id: ki-policy-v1-000001
    evidence_id: evidence-ki-policy-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-policy-v1-000001

## Assertion

- Subject: [Controls](../concepts/concept-controls.md)
- Predicate: `requires`
- Object: Staff must confirm badge access before entering the restricted area.

## Connected Knowledge

- Knowledge piece: [ki-policy-v1-000001](../knowledge/ki-policy-v1-000001.md)
- Evidence: [evidence-ki-policy-v1-000001](../evidence/evidence-ki-policy-v1-000001.md)
- Decision rule: [Controls](../rules/rule-ki-policy-v1-000001-requires.md)
