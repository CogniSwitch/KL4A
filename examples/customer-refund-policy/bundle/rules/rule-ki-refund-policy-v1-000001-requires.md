---
type: SOP Decision Rule
title: Refund Eligibility
description: Customers must submit refund requests within 30 days of purchase.
resource: ../knowledge/ki-refund-policy-v1-000001.md
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
    id: rule-ki-refund-policy-v1-000001-requires
    type: SOP Decision Rule
    title: Refund Eligibility
    knowledge_item_id: ki-refund-policy-v1-000001
    source_id: refund-policy
    section_id: section-refund-policy-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_refund_eligibility
      action: requires
      label: Customers must submit refund requests within 30 days of purchase.
    evidence_id: evidence-ki-refund-policy-v1-000001
    relation_id: kr-ki-refund-policy-v1-000001
    okf_path: rules/rule-ki-refund-policy-v1-000001-requires.md
  knowledge_piece: ../knowledge/ki-refund-policy-v1-000001.md
  knowledge_relation: ../relations/kr-ki-refund-policy-v1-000001.md
  evidence: ../evidence/evidence-ki-refund-policy-v1-000001.md
---
# Refund Eligibility

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_refund_eligibility`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-refund-policy-v1-000001](../knowledge/ki-refund-policy-v1-000001.md)
- Knowledge relation: [kr-ki-refund-policy-v1-000001](../relations/kr-ki-refund-policy-v1-000001.md)
- Evidence: [evidence-ki-refund-policy-v1-000001](../evidence/evidence-ki-refund-policy-v1-000001.md)
