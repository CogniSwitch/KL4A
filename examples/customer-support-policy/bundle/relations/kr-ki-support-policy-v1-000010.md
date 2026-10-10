---
type: SOP Knowledge Relation
title: kr-ki-support-policy-v1-000010
description: 4. Final Sale Items requires Custom or personalized items must be treated
  as final.
resource: ../knowledge/ki-support-policy-v1-000010.md
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
    id: kr-ki-support-policy-v1-000010
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
      id: object-custom-or-personalized-items-must-be-treated-as-final
      text: Custom or personalized items must be treated as final sale once production
        has begun.
      label: Custom or personalized items must be treated as final
    knowledge_piece_id: ki-support-policy-v1-000010
    evidence_id: evidence-ki-support-policy-v1-000010
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-support-policy-v1-000010

## Assertion

- Subject: [4. Final Sale Items](../concepts/concept-4-final-sale-items.md)
- Predicate: `requires`
- Object: Custom or personalized items must be treated as final sale once production has begun.

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000010](../knowledge/ki-support-policy-v1-000010.md)
- Evidence: [evidence-ki-support-policy-v1-000010](../evidence/evidence-ki-support-policy-v1-000010.md)
- Decision rule: [4. Final Sale Items](../rules/rule-ki-support-policy-v1-000010-requires.md)
