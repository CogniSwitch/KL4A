---
type: SOP Decision Rule
title: 7. Power Tools
description: Power Tools must include original packaging and all accessories to avoid
  a 15% restocking fee.
resource: ../knowledge/ki-category-policy-v1-000014.md
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
    id: rule-ki-category-policy-v1-000014-requires
    type: SOP Decision Rule
    title: 7. Power Tools
    knowledge_item_id: ki-category-policy-v1-000014
    source_id: category-policy
    section_id: section-category-policy-008
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_7_power_tools
      action: requires
      label: Power Tools must include original packaging and all accessories to avoid
        a 15% restocking fee.
    evidence_id: evidence-ki-category-policy-v1-000014
    relation_id: kr-ki-category-policy-v1-000014
    okf_path: rules/rule-ki-category-policy-v1-000014-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000014.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000014.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000014.md
---
# 7. Power Tools

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_7_power_tools`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000014](../knowledge/ki-category-policy-v1-000014.md)
- Knowledge relation: [kr-ki-category-policy-v1-000014](../relations/kr-ki-category-policy-v1-000014.md)
- Evidence: [evidence-ki-category-policy-v1-000014](../evidence/evidence-ki-category-policy-v1-000014.md)
