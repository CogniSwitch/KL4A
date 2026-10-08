---
type: SOP Decision Rule
title: Refund Approval
description: Refund requests above $1,000 must receive finance approval before processing.
resource: ../knowledge/ki-refund-policy-v1-000003.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sources:
- id: src-refund-policy
  title: refund policy
  resource: ../sources/refund-policy.md
sopkb:
  rule:
    id: rule-ki-refund-policy-v1-000003-requires
    type: SOP Decision Rule
    title: Refund Approval
    knowledge_item_id: ki-refund-policy-v1-000003
    source_id: refund-policy
    section_id: section-refund-policy-003
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_refund_approval
      action: requires
      label: Refund requests above $1,000 must receive finance approval before processing.
    evidence_id: evidence-ki-refund-policy-v1-000003
    relation_id: kr-ki-refund-policy-v1-000003
    okf_path: rules/rule-ki-refund-policy-v1-000003-requires.md
  knowledge_piece: ../knowledge/ki-refund-policy-v1-000003.md
  knowledge_relation: ../relations/kr-ki-refund-policy-v1-000003.md
  evidence: ../evidence/evidence-ki-refund-policy-v1-000003.md
---
# Refund Approval

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_refund_approval`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-refund-policy-v1-000003](../knowledge/ki-refund-policy-v1-000003.md)
- Knowledge relation: [kr-ki-refund-policy-v1-000003](../relations/kr-ki-refund-policy-v1-000003.md)
- Evidence: [evidence-ki-refund-policy-v1-000003](../evidence/evidence-ki-refund-policy-v1-000003.md)
