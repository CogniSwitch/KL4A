---
type: SOP Decision Rule
title: 9. Laptop Computers
description: Laptop Computers must include original packaging and all accessories
  to avoid a 15% restocking fee.
resource: ../knowledge/ki-category-policy-v1-000018.md
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
    id: rule-ki-category-policy-v1-000018-requires
    type: SOP Decision Rule
    title: 9. Laptop Computers
    knowledge_item_id: ki-category-policy-v1-000018
    source_id: category-policy
    section_id: section-category-policy-010
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_9_laptop_computers
      action: requires
      label: Laptop Computers must include original packaging and all accessories
        to avoid a 15% restocking fee.
    evidence_id: evidence-ki-category-policy-v1-000018
    relation_id: kr-ki-category-policy-v1-000018
    okf_path: rules/rule-ki-category-policy-v1-000018-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000018.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000018.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000018.md
---
# 9. Laptop Computers

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_9_laptop_computers`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000018](../knowledge/ki-category-policy-v1-000018.md)
- Knowledge relation: [kr-ki-category-policy-v1-000018](../relations/kr-ki-category-policy-v1-000018.md)
- Evidence: [evidence-ki-category-policy-v1-000018](../evidence/evidence-ki-category-policy-v1-000018.md)
