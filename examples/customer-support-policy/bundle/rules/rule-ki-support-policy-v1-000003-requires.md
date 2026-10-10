---
type: SOP Decision Rule
title: 1. General Return Window
description: Items returned after day 45 must not receive any refund or credit.
resource: ../knowledge/ki-support-policy-v1-000003.md
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
    id: rule-ki-support-policy-v1-000003-requires
    type: SOP Decision Rule
    title: 1. General Return Window
    knowledge_item_id: ki-support-policy-v1-000003
    source_id: support-policy
    section_id: section-support-policy-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_1_general_return_window
      action: requires
      label: Items returned after day 45 must not receive any refund or credit.
    evidence_id: evidence-ki-support-policy-v1-000003
    relation_id: kr-ki-support-policy-v1-000003
    okf_path: rules/rule-ki-support-policy-v1-000003-requires.md
  knowledge_piece: ../knowledge/ki-support-policy-v1-000003.md
  knowledge_relation: ../relations/kr-ki-support-policy-v1-000003.md
  evidence: ../evidence/evidence-ki-support-policy-v1-000003.md
---
# 1. General Return Window

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_1_general_return_window`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000003](../knowledge/ki-support-policy-v1-000003.md)
- Knowledge relation: [kr-ki-support-policy-v1-000003](../relations/kr-ki-support-policy-v1-000003.md)
- Evidence: [evidence-ki-support-policy-v1-000003](../evidence/evidence-ki-support-policy-v1-000003.md)
