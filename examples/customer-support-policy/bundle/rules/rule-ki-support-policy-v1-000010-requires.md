---
type: SOP Decision Rule
title: 4. Final Sale Items
description: Custom or personalized items must be treated as final sale once production
  has begun.
resource: ../knowledge/ki-support-policy-v1-000010.md
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
    id: rule-ki-support-policy-v1-000010-requires
    type: SOP Decision Rule
    title: 4. Final Sale Items
    knowledge_item_id: ki-support-policy-v1-000010
    source_id: support-policy
    section_id: section-support-policy-005
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_4_final_sale_items
      action: requires
      label: Custom or personalized items must be treated as final sale once production
        has begun.
    evidence_id: evidence-ki-support-policy-v1-000010
    relation_id: kr-ki-support-policy-v1-000010
    okf_path: rules/rule-ki-support-policy-v1-000010-requires.md
  knowledge_piece: ../knowledge/ki-support-policy-v1-000010.md
  knowledge_relation: ../relations/kr-ki-support-policy-v1-000010.md
  evidence: ../evidence/evidence-ki-support-policy-v1-000010.md
---
# 4. Final Sale Items

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_4_final_sale_items`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000010](../knowledge/ki-support-policy-v1-000010.md)
- Knowledge relation: [kr-ki-support-policy-v1-000010](../relations/kr-ki-support-policy-v1-000010.md)
- Evidence: [evidence-ki-support-policy-v1-000010](../evidence/evidence-ki-support-policy-v1-000010.md)
