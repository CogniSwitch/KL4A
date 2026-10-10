---
type: SOP Decision Rule
title: 11. Mobile Phones
description: Mobile Phones must be returned within 14 days of delivery to qualify
  for a refund.
resource: ../knowledge/ki-category-policy-v1-000021.md
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
    id: rule-ki-category-policy-v1-000021-requires
    type: SOP Decision Rule
    title: 11. Mobile Phones
    knowledge_item_id: ki-category-policy-v1-000021
    source_id: category-policy
    section_id: section-category-policy-012
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_11_mobile_phones
      action: requires
      label: Mobile Phones must be returned within 14 days of delivery to qualify
        for a refund.
    evidence_id: evidence-ki-category-policy-v1-000021
    relation_id: kr-ki-category-policy-v1-000021
    okf_path: rules/rule-ki-category-policy-v1-000021-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000021.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000021.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000021.md
---
# 11. Mobile Phones

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_11_mobile_phones`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000021](../knowledge/ki-category-policy-v1-000021.md)
- Knowledge relation: [kr-ki-category-policy-v1-000021](../relations/kr-ki-category-policy-v1-000021.md)
- Evidence: [evidence-ki-category-policy-v1-000021](../evidence/evidence-ki-category-policy-v1-000021.md)
