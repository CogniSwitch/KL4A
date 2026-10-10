---
type: SOP Decision Rule
title: 2. Major Appliances
description: Major Appliances must include original packaging and all accessories
  to avoid a 20% restocking fee.
resource: ../knowledge/ki-category-policy-v1-000004.md
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
    id: rule-ki-category-policy-v1-000004-requires
    type: SOP Decision Rule
    title: 2. Major Appliances
    knowledge_item_id: ki-category-policy-v1-000004
    source_id: category-policy
    section_id: section-category-policy-003
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_2_major_appliances
      action: requires
      label: Major Appliances must include original packaging and all accessories
        to avoid a 20% restocking fee.
    evidence_id: evidence-ki-category-policy-v1-000004
    relation_id: kr-ki-category-policy-v1-000004
    okf_path: rules/rule-ki-category-policy-v1-000004-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000004.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000004.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000004.md
---
# 2. Major Appliances

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_2_major_appliances`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000004](../knowledge/ki-category-policy-v1-000004.md)
- Knowledge relation: [kr-ki-category-policy-v1-000004](../relations/kr-ki-category-policy-v1-000004.md)
- Evidence: [evidence-ki-category-policy-v1-000004](../evidence/evidence-ki-category-policy-v1-000004.md)
