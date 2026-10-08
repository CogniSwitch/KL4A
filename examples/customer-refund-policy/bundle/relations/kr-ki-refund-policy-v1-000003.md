---
type: SOP Knowledge Relation
title: kr-ki-refund-policy-v1-000003
description: Refund Approval requires Refund requests above $1,000 must receive finance
  approval before.
resource: ../knowledge/ki-refund-policy-v1-000003.md
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
    id: kr-ki-refund-policy-v1-000003
    type: Knowledge Relation
    subject:
      id: concept-refund-approval
      label: Refund Approval
      text: Refund Approval
      okf_path: concepts/concept-refund-approval.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-refund-requests-above-1-000-must-receive-finance-approval-before
      text: Refund requests above $1,000 must receive finance approval before processing.
      label: Refund requests above $1,000 must receive finance approval before
    knowledge_piece_id: ki-refund-policy-v1-000003
    evidence_id: evidence-ki-refund-policy-v1-000003
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-refund-policy-v1-000003

## Assertion

- Subject: [Refund Approval](../concepts/concept-refund-approval.md)
- Predicate: `requires`
- Object: Refund requests above $1,000 must receive finance approval before processing.

## Connected Knowledge

- Knowledge piece: [ki-refund-policy-v1-000003](../knowledge/ki-refund-policy-v1-000003.md)
- Evidence: [evidence-ki-refund-policy-v1-000003](../evidence/evidence-ki-refund-policy-v1-000003.md)
- Decision rule: [Refund Approval](../rules/rule-ki-refund-policy-v1-000003-requires.md)
