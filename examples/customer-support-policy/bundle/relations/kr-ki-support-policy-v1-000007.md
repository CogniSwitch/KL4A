---
type: SOP Knowledge Relation
title: kr-ki-support-policy-v1-000007
description: 3. Electronics Exceptions requires Electronics must be returned within
  14 days of delivery.
resource: ../knowledge/ki-support-policy-v1-000007.md
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
    id: kr-ki-support-policy-v1-000007
    type: Knowledge Relation
    subject:
      id: concept-3-electronics-exceptions
      label: 3. Electronics Exceptions
      text: 3. Electronics Exceptions
      okf_path: concepts/concept-3-electronics-exceptions.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-electronics-must-be-returned-within-14-days-of-delivery
      text: Electronics must be returned within 14 days of delivery, regardless of
        the general return window described in Section 1.
      label: Electronics must be returned within 14 days of delivery
    knowledge_piece_id: ki-support-policy-v1-000007
    evidence_id: evidence-ki-support-policy-v1-000007
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-support-policy-v1-000007

## Assertion

- Subject: [3. Electronics Exceptions](../concepts/concept-3-electronics-exceptions.md)
- Predicate: `requires`
- Object: Electronics must be returned within 14 days of delivery, regardless of the general return window described in Section 1.

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000007](../knowledge/ki-support-policy-v1-000007.md)
- Evidence: [evidence-ki-support-policy-v1-000007](../evidence/evidence-ki-support-policy-v1-000007.md)
- Decision rule: [3. Electronics Exceptions](../rules/rule-ki-support-policy-v1-000007-requires.md)
