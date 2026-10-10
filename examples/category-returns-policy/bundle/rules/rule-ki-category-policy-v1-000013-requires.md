---
type: SOP Decision Rule
title: 7. Power Tools
description: Power Tools must be returned within 30 days of delivery to qualify for
  a refund.
resource: ../knowledge/ki-category-policy-v1-000013.md
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
    id: rule-ki-category-policy-v1-000013-requires
    type: SOP Decision Rule
    title: 7. Power Tools
    knowledge_item_id: ki-category-policy-v1-000013
    source_id: category-policy
    section_id: section-category-policy-008
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_7_power_tools
      action: requires
      label: Power Tools must be returned within 30 days of delivery to qualify for
        a refund.
    evidence_id: evidence-ki-category-policy-v1-000013
    relation_id: kr-ki-category-policy-v1-000013
    okf_path: rules/rule-ki-category-policy-v1-000013-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000013.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000013.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000013.md
---
# 7. Power Tools

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_7_power_tools`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000013](../knowledge/ki-category-policy-v1-000013.md)
- Knowledge relation: [kr-ki-category-policy-v1-000013](../relations/kr-ki-category-policy-v1-000013.md)
- Evidence: [evidence-ki-category-policy-v1-000013](../evidence/evidence-ki-category-policy-v1-000013.md)
