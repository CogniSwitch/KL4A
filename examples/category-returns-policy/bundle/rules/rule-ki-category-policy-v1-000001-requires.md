---
type: SOP Decision Rule
title: 1. Small Appliances
description: Small Appliances must be returned within 21 days of delivery to qualify
  for a refund.
resource: ../knowledge/ki-category-policy-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sources:
- id: src-category-policy
  title: category policy
  resource: ../sources/category-policy.md
sopkb:
  rule:
    id: rule-ki-category-policy-v1-000001-requires
    type: SOP Decision Rule
    title: 1. Small Appliances
    knowledge_item_id: ki-category-policy-v1-000001
    source_id: category-policy
    section_id: section-category-policy-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_1_small_appliances
      action: requires
      label: Small Appliances must be returned within 21 days of delivery to qualify
        for a refund.
    evidence_id: evidence-ki-category-policy-v1-000001
    relation_id: kr-ki-category-policy-v1-000001
    okf_path: rules/rule-ki-category-policy-v1-000001-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000001.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000001.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000001.md
---
# 1. Small Appliances

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_1_small_appliances`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000001](../knowledge/ki-category-policy-v1-000001.md)
- Knowledge relation: [kr-ki-category-policy-v1-000001](../relations/kr-ki-category-policy-v1-000001.md)
- Evidence: [evidence-ki-category-policy-v1-000001](../evidence/evidence-ki-category-policy-v1-000001.md)
