---
type: SOP Decision Rule
title: 6. Non-Prescription Sunglasses
description: Non-Prescription Sunglasses must be returned within 30 days of delivery
  to qualify for a refund.
resource: ../knowledge/ki-category-policy-v1-000011.md
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
    id: rule-ki-category-policy-v1-000011-requires
    type: SOP Decision Rule
    title: 6. Non-Prescription Sunglasses
    knowledge_item_id: ki-category-policy-v1-000011
    source_id: category-policy
    section_id: section-category-policy-007
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_6_non_prescription_sunglasses
      action: requires
      label: Non-Prescription Sunglasses must be returned within 30 days of delivery
        to qualify for a refund.
    evidence_id: evidence-ki-category-policy-v1-000011
    relation_id: kr-ki-category-policy-v1-000011
    okf_path: rules/rule-ki-category-policy-v1-000011-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000011.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000011.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000011.md
---
# 6. Non-Prescription Sunglasses

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_6_non_prescription_sunglasses`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000011](../knowledge/ki-category-policy-v1-000011.md)
- Knowledge relation: [kr-ki-category-policy-v1-000011](../relations/kr-ki-category-policy-v1-000011.md)
- Evidence: [evidence-ki-category-policy-v1-000011](../evidence/evidence-ki-category-policy-v1-000011.md)
