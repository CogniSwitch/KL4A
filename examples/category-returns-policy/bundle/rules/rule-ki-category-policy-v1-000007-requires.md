---
type: SOP Decision Rule
title: 4. Outdoor Furniture
description: Outdoor Furniture must be returned within 14 days of delivery to qualify
  for a refund.
resource: ../knowledge/ki-category-policy-v1-000007.md
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
    id: rule-ki-category-policy-v1-000007-requires
    type: SOP Decision Rule
    title: 4. Outdoor Furniture
    knowledge_item_id: ki-category-policy-v1-000007
    source_id: category-policy
    section_id: section-category-policy-005
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_4_outdoor_furniture
      action: requires
      label: Outdoor Furniture must be returned within 14 days of delivery to qualify
        for a refund.
    evidence_id: evidence-ki-category-policy-v1-000007
    relation_id: kr-ki-category-policy-v1-000007
    okf_path: rules/rule-ki-category-policy-v1-000007-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000007.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000007.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000007.md
---
# 4. Outdoor Furniture

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_4_outdoor_furniture`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000007](../knowledge/ki-category-policy-v1-000007.md)
- Knowledge relation: [kr-ki-category-policy-v1-000007](../relations/kr-ki-category-policy-v1-000007.md)
- Evidence: [evidence-ki-category-policy-v1-000007](../evidence/evidence-ki-category-policy-v1-000007.md)
