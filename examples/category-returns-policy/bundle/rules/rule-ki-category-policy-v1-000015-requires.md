---
type: SOP Decision Rule
title: 8. Hand Tools
description: Hand Tools must be returned within 60 days of delivery to qualify for
  a refund.
resource: ../knowledge/ki-category-policy-v1-000015.md
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
    id: rule-ki-category-policy-v1-000015-requires
    type: SOP Decision Rule
    title: 8. Hand Tools
    knowledge_item_id: ki-category-policy-v1-000015
    source_id: category-policy
    section_id: section-category-policy-009
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_8_hand_tools
      action: requires
      label: Hand Tools must be returned within 60 days of delivery to qualify for
        a refund.
    evidence_id: evidence-ki-category-policy-v1-000015
    relation_id: kr-ki-category-policy-v1-000015
    okf_path: rules/rule-ki-category-policy-v1-000015-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000015.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000015.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000015.md
---
# 8. Hand Tools

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_8_hand_tools`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000015](../knowledge/ki-category-policy-v1-000015.md)
- Knowledge relation: [kr-ki-category-policy-v1-000015](../relations/kr-ki-category-policy-v1-000015.md)
- Evidence: [evidence-ki-category-policy-v1-000015](../evidence/evidence-ki-category-policy-v1-000015.md)
