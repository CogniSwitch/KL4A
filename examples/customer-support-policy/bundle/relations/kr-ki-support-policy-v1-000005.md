---
type: SOP Knowledge Relation
title: kr-ki-support-policy-v1-000005
description: 2. Condition Requirements requires Staff must apply a 15% restocking
  fee to items.
resource: ../knowledge/ki-support-policy-v1-000005.md
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
    id: kr-ki-support-policy-v1-000005
    type: Knowledge Relation
    subject:
      id: concept-2-condition-requirements
      label: 2. Condition Requirements
      text: 2. Condition Requirements
      okf_path: concepts/concept-2-condition-requirements.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-staff-must-apply-a-15-restocking-fee-to-items
      text: Staff must apply a 15% restocking fee to items missing original packaging.
      label: Staff must apply a 15% restocking fee to items
    knowledge_piece_id: ki-support-policy-v1-000005
    evidence_id: evidence-ki-support-policy-v1-000005
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-support-policy-v1-000005

## Assertion

- Subject: [2. Condition Requirements](../concepts/concept-2-condition-requirements.md)
- Predicate: `requires`
- Object: Staff must apply a 15% restocking fee to items missing original packaging.

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000005](../knowledge/ki-support-policy-v1-000005.md)
- Evidence: [evidence-ki-support-policy-v1-000005](../evidence/evidence-ki-support-policy-v1-000005.md)
- Decision rule: [2. Condition Requirements](../rules/rule-ki-support-policy-v1-000005-requires.md)
