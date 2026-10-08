---
type: SOP Decision Rule
title: 6. Non-Prescription Sunglasses
description: Non-Prescription Sunglasses must include original packaging and all accessories
  to avoid a 10% restocking fee.
resource: ../knowledge/ki-category-policy-v1-000012.md
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
    id: rule-ki-category-policy-v1-000012-requires
    type: SOP Decision Rule
    title: 6. Non-Prescription Sunglasses
    knowledge_item_id: ki-category-policy-v1-000012
    source_id: category-policy
    section_id: section-category-policy-007
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_6_non_prescription_sunglasses
      action: requires
      label: Non-Prescription Sunglasses must include original packaging and all accessories
        to avoid a 10% restocking fee.
    evidence_id: evidence-ki-category-policy-v1-000012
    relation_id: kr-ki-category-policy-v1-000012
    okf_path: rules/rule-ki-category-policy-v1-000012-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000012.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000012.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000012.md
---
# 6. Non-Prescription Sunglasses

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_6_non_prescription_sunglasses`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000012](../knowledge/ki-category-policy-v1-000012.md)
- Knowledge relation: [kr-ki-category-policy-v1-000012](../relations/kr-ki-category-policy-v1-000012.md)
- Evidence: [evidence-ki-category-policy-v1-000012](../evidence/evidence-ki-category-policy-v1-000012.md)
