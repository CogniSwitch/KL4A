---
type: SOP Decision Rule
title: 5. Damaged or Defective Items
description: Customers must report damaged or defective items within 48 hours of delivery
  to qualify for a free replacement or full refund including return shipping costs.
resource: ../knowledge/ki-support-policy-v1-000011.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sources:
- id: src-support-policy
  title: support policy
  resource: ../sources/support-policy.md
sopkb:
  rule:
    id: rule-ki-support-policy-v1-000011-requires
    type: SOP Decision Rule
    title: 5. Damaged or Defective Items
    knowledge_item_id: ki-support-policy-v1-000011
    source_id: support-policy
    section_id: section-support-policy-006
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_5_damaged_or_defective_items
      action: requires
      label: Customers must report damaged or defective items within 48 hours of delivery
        to qualify for a free replacement or full refund including return shipping
        costs.
    evidence_id: evidence-ki-support-policy-v1-000011
    relation_id: kr-ki-support-policy-v1-000011
    okf_path: rules/rule-ki-support-policy-v1-000011-requires.md
  knowledge_piece: ../knowledge/ki-support-policy-v1-000011.md
  knowledge_relation: ../relations/kr-ki-support-policy-v1-000011.md
  evidence: ../evidence/evidence-ki-support-policy-v1-000011.md
---
# 5. Damaged or Defective Items

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_5_damaged_or_defective_items`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000011](../knowledge/ki-support-policy-v1-000011.md)
- Knowledge relation: [kr-ki-support-policy-v1-000011](../relations/kr-ki-support-policy-v1-000011.md)
- Evidence: [evidence-ki-support-policy-v1-000011](../evidence/evidence-ki-support-policy-v1-000011.md)
