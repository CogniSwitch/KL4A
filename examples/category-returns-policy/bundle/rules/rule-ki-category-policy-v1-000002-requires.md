---
type: SOP Decision Rule
title: 1. Small Appliances
description: Small Appliances must include original packaging and all accessories
  to avoid a 10% restocking fee.
resource: ../knowledge/ki-category-policy-v1-000002.md
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
    id: rule-ki-category-policy-v1-000002-requires
    type: SOP Decision Rule
    title: 1. Small Appliances
    knowledge_item_id: ki-category-policy-v1-000002
    source_id: category-policy
    section_id: section-category-policy-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_1_small_appliances
      action: requires
      label: Small Appliances must include original packaging and all accessories
        to avoid a 10% restocking fee.
    evidence_id: evidence-ki-category-policy-v1-000002
    relation_id: kr-ki-category-policy-v1-000002
    okf_path: rules/rule-ki-category-policy-v1-000002-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000002.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000002.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000002.md
---
# 1. Small Appliances

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_1_small_appliances`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000002](../knowledge/ki-category-policy-v1-000002.md)
- Knowledge relation: [kr-ki-category-policy-v1-000002](../relations/kr-ki-category-policy-v1-000002.md)
- Evidence: [evidence-ki-category-policy-v1-000002](../evidence/evidence-ki-category-policy-v1-000002.md)
