---
type: SOP Decision Rule
title: 1. General Return Window
description: Customers must initiate a return within 30 days of delivery to receive
  a full refund.
resource: ../knowledge/ki-support-policy-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sources:
- id: src-support-policy
  title: support policy
  resource: ../sources/support-policy.md
sopkb:
  rule:
    id: rule-ki-support-policy-v1-000001-requires
    type: SOP Decision Rule
    title: 1. General Return Window
    knowledge_item_id: ki-support-policy-v1-000001
    source_id: support-policy
    section_id: section-support-policy-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_1_general_return_window
      action: requires
      label: Customers must initiate a return within 30 days of delivery to receive
        a full refund.
    evidence_id: evidence-ki-support-policy-v1-000001
    relation_id: kr-ki-support-policy-v1-000001
    okf_path: rules/rule-ki-support-policy-v1-000001-requires.md
  knowledge_piece: ../knowledge/ki-support-policy-v1-000001.md
  knowledge_relation: ../relations/kr-ki-support-policy-v1-000001.md
  evidence: ../evidence/evidence-ki-support-policy-v1-000001.md
---
# 1. General Return Window

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_1_general_return_window`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000001](../knowledge/ki-support-policy-v1-000001.md)
- Knowledge relation: [kr-ki-support-policy-v1-000001](../relations/kr-ki-support-policy-v1-000001.md)
- Evidence: [evidence-ki-support-policy-v1-000001](../evidence/evidence-ki-support-policy-v1-000001.md)
