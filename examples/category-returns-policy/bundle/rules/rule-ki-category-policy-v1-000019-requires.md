---
type: SOP Decision Rule
title: 10. Desktop Computers
description: Desktop Computers must be returned within 30 days of delivery to qualify
  for a refund.
resource: ../knowledge/ki-category-policy-v1-000019.md
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
    id: rule-ki-category-policy-v1-000019-requires
    type: SOP Decision Rule
    title: 10. Desktop Computers
    knowledge_item_id: ki-category-policy-v1-000019
    source_id: category-policy
    section_id: section-category-policy-011
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_10_desktop_computers
      action: requires
      label: Desktop Computers must be returned within 30 days of delivery to qualify
        for a refund.
    evidence_id: evidence-ki-category-policy-v1-000019
    relation_id: kr-ki-category-policy-v1-000019
    okf_path: rules/rule-ki-category-policy-v1-000019-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000019.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000019.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000019.md
---
# 10. Desktop Computers

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_10_desktop_computers`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000019](../knowledge/ki-category-policy-v1-000019.md)
- Knowledge relation: [kr-ki-category-policy-v1-000019](../relations/kr-ki-category-policy-v1-000019.md)
- Evidence: [evidence-ki-category-policy-v1-000019](../evidence/evidence-ki-category-policy-v1-000019.md)
