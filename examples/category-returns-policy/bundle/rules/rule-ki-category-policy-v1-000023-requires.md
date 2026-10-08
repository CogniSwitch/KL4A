---
type: SOP Decision Rule
title: 12. Tablet Devices
description: Tablet Devices must be returned within 21 days of delivery to qualify
  for a refund.
resource: ../knowledge/ki-category-policy-v1-000023.md
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
    id: rule-ki-category-policy-v1-000023-requires
    type: SOP Decision Rule
    title: 12. Tablet Devices
    knowledge_item_id: ki-category-policy-v1-000023
    source_id: category-policy
    section_id: section-category-policy-013
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_12_tablet_devices
      action: requires
      label: Tablet Devices must be returned within 21 days of delivery to qualify
        for a refund.
    evidence_id: evidence-ki-category-policy-v1-000023
    relation_id: kr-ki-category-policy-v1-000023
    okf_path: rules/rule-ki-category-policy-v1-000023-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000023.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000023.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000023.md
---
# 12. Tablet Devices

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_12_tablet_devices`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000023](../knowledge/ki-category-policy-v1-000023.md)
- Knowledge relation: [kr-ki-category-policy-v1-000023](../relations/kr-ki-category-policy-v1-000023.md)
- Evidence: [evidence-ki-category-policy-v1-000023](../evidence/evidence-ki-category-policy-v1-000023.md)
