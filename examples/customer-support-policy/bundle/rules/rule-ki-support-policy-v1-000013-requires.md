---
type: SOP Decision Rule
title: 6. Refund Processing
description: Staff must issue approved refunds to the original payment method within
  5-7 business days of the returned item being received and inspected.
resource: ../knowledge/ki-support-policy-v1-000013.md
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
    id: rule-ki-support-policy-v1-000013-requires
    type: SOP Decision Rule
    title: 6. Refund Processing
    knowledge_item_id: ki-support-policy-v1-000013
    source_id: support-policy
    section_id: section-support-policy-007
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_6_refund_processing
      action: requires
      label: Staff must issue approved refunds to the original payment method within
        5-7 business days of the returned item being received and inspected.
    evidence_id: evidence-ki-support-policy-v1-000013
    relation_id: kr-ki-support-policy-v1-000013
    okf_path: rules/rule-ki-support-policy-v1-000013-requires.md
  knowledge_piece: ../knowledge/ki-support-policy-v1-000013.md
  knowledge_relation: ../relations/kr-ki-support-policy-v1-000013.md
  evidence: ../evidence/evidence-ki-support-policy-v1-000013.md
---
# 6. Refund Processing

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_6_refund_processing`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000013](../knowledge/ki-support-policy-v1-000013.md)
- Knowledge relation: [kr-ki-support-policy-v1-000013](../relations/kr-ki-support-policy-v1-000013.md)
- Evidence: [evidence-ki-support-policy-v1-000013](../evidence/evidence-ki-support-policy-v1-000013.md)
