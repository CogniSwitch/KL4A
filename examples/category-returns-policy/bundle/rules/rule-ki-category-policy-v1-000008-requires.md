---
type: SOP Decision Rule
title: 4. Outdoor Furniture
description: Outdoor Furniture must include original packaging and all accessories
  to avoid a 25% restocking fee.
resource: ../knowledge/ki-category-policy-v1-000008.md
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
    id: rule-ki-category-policy-v1-000008-requires
    type: SOP Decision Rule
    title: 4. Outdoor Furniture
    knowledge_item_id: ki-category-policy-v1-000008
    source_id: category-policy
    section_id: section-category-policy-005
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_4_outdoor_furniture
      action: requires
      label: Outdoor Furniture must include original packaging and all accessories
        to avoid a 25% restocking fee.
    evidence_id: evidence-ki-category-policy-v1-000008
    relation_id: kr-ki-category-policy-v1-000008
    okf_path: rules/rule-ki-category-policy-v1-000008-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000008.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000008.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000008.md
---
# 4. Outdoor Furniture

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_4_outdoor_furniture`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000008](../knowledge/ki-category-policy-v1-000008.md)
- Knowledge relation: [kr-ki-category-policy-v1-000008](../relations/kr-ki-category-policy-v1-000008.md)
- Evidence: [evidence-ki-category-policy-v1-000008](../evidence/evidence-ki-category-policy-v1-000008.md)
