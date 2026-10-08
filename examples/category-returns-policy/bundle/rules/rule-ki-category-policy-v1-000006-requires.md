---
type: SOP Decision Rule
title: 3. Indoor Furniture
description: Indoor Furniture must include original packaging and all accessories
  to avoid a 15% restocking fee.
resource: ../knowledge/ki-category-policy-v1-000006.md
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
    id: rule-ki-category-policy-v1-000006-requires
    type: SOP Decision Rule
    title: 3. Indoor Furniture
    knowledge_item_id: ki-category-policy-v1-000006
    source_id: category-policy
    section_id: section-category-policy-004
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_3_indoor_furniture
      action: requires
      label: Indoor Furniture must include original packaging and all accessories
        to avoid a 15% restocking fee.
    evidence_id: evidence-ki-category-policy-v1-000006
    relation_id: kr-ki-category-policy-v1-000006
    okf_path: rules/rule-ki-category-policy-v1-000006-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000006.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000006.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000006.md
---
# 3. Indoor Furniture

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_3_indoor_furniture`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000006](../knowledge/ki-category-policy-v1-000006.md)
- Knowledge relation: [kr-ki-category-policy-v1-000006](../relations/kr-ki-category-policy-v1-000006.md)
- Evidence: [evidence-ki-category-policy-v1-000006](../evidence/evidence-ki-category-policy-v1-000006.md)
