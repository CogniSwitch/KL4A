---
type: SOP Knowledge Relation
title: kr-ki-support-policy-v1-000009
description: 4. Final Sale Items requires Clearance items marked "Final Sale" at checkout
  must not.
resource: ../knowledge/ki-support-policy-v1-000009.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sources:
- id: src-support-policy
  title: support policy
  resource: ../sources/support-policy.md
sopkb:
  relation:
    id: kr-ki-support-policy-v1-000009
    type: Knowledge Relation
    subject:
      id: concept-4-final-sale-items
      label: 4. Final Sale Items
      text: 4. Final Sale Items
      okf_path: concepts/concept-4-final-sale-items.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-clearance-items-marked-final-sale-at-checkout-must-not
      text: Clearance items marked "Final Sale" at checkout must not be accepted for
        return or exchange.
      label: Clearance items marked "Final Sale" at checkout must not
    knowledge_piece_id: ki-support-policy-v1-000009
    evidence_id: evidence-ki-support-policy-v1-000009
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-support-policy-v1-000009

## Assertion

- Subject: [4. Final Sale Items](../concepts/concept-4-final-sale-items.md)
- Predicate: `requires`
- Object: Clearance items marked "Final Sale" at checkout must not be accepted for return or exchange.

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000009](../knowledge/ki-support-policy-v1-000009.md)
- Evidence: [evidence-ki-support-policy-v1-000009](../evidence/evidence-ki-support-policy-v1-000009.md)
- Decision rule: [4. Final Sale Items](../rules/rule-ki-support-policy-v1-000009-requires.md)
