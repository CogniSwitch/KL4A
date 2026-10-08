---
type: SOP Decision Rule
title: 5. Damaged or Defective Items
description: Reports made after 48 hours must be handled under the general return
  window described in Section 1, with return shipping paid by the customer.
resource: ../knowledge/ki-support-policy-v1-000012.md
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
    id: rule-ki-support-policy-v1-000012-requires
    type: SOP Decision Rule
    title: 5. Damaged or Defective Items
    knowledge_item_id: ki-support-policy-v1-000012
    source_id: support-policy
    section_id: section-support-policy-006
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_5_damaged_or_defective_items
      action: requires
      label: Reports made after 48 hours must be handled under the general return
        window described in Section 1, with return shipping paid by the customer.
    evidence_id: evidence-ki-support-policy-v1-000012
    relation_id: kr-ki-support-policy-v1-000012
    okf_path: rules/rule-ki-support-policy-v1-000012-requires.md
  knowledge_piece: ../knowledge/ki-support-policy-v1-000012.md
  knowledge_relation: ../relations/kr-ki-support-policy-v1-000012.md
  evidence: ../evidence/evidence-ki-support-policy-v1-000012.md
---
# 5. Damaged or Defective Items

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_5_damaged_or_defective_items`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000012](../knowledge/ki-support-policy-v1-000012.md)
- Knowledge relation: [kr-ki-support-policy-v1-000012](../relations/kr-ki-support-policy-v1-000012.md)
- Evidence: [evidence-ki-support-policy-v1-000012](../evidence/evidence-ki-support-policy-v1-000012.md)
