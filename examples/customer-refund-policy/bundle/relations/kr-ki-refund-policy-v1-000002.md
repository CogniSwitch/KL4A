---
type: SOP Knowledge Relation
title: kr-ki-refund-policy-v1-000002
description: Refund Eligibility requires Digital products must not have been downloaded
  to qualify.
resource: ../knowledge/ki-refund-policy-v1-000002.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sources:
- id: src-refund-policy
  title: refund policy
  resource: ../sources/refund-policy.md
sopkb:
  relation:
    id: kr-ki-refund-policy-v1-000002
    type: Knowledge Relation
    subject:
      id: concept-refund-eligibility
      label: Refund Eligibility
      text: Refund Eligibility
      okf_path: concepts/concept-refund-eligibility.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-digital-products-must-not-have-been-downloaded-to-qualify
      text: Digital products must not have been downloaded to qualify for a refund.
      label: Digital products must not have been downloaded to qualify
    knowledge_piece_id: ki-refund-policy-v1-000002
    evidence_id: evidence-ki-refund-policy-v1-000002
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-refund-policy-v1-000002

## Assertion

- Subject: [Refund Eligibility](../concepts/concept-refund-eligibility.md)
- Predicate: `requires`
- Object: Digital products must not have been downloaded to qualify for a refund.

## Connected Knowledge

- Knowledge piece: [ki-refund-policy-v1-000002](../knowledge/ki-refund-policy-v1-000002.md)
- Evidence: [evidence-ki-refund-policy-v1-000002](../evidence/evidence-ki-refund-policy-v1-000002.md)
- Decision rule: [Refund Eligibility](../rules/rule-ki-refund-policy-v1-000002-requires.md)
