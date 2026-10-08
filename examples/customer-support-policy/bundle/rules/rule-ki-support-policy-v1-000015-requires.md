---
type: SOP Decision Rule
title: 7. Exchanges
description: Staff must waive the restocking fee described in Section 2 for exchanges
  of the same item in a different size or color, provided the request is made within
  the general return window described in Section 1.
resource: ../knowledge/ki-support-policy-v1-000015.md
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
    id: rule-ki-support-policy-v1-000015-requires
    type: SOP Decision Rule
    title: 7. Exchanges
    knowledge_item_id: ki-support-policy-v1-000015
    source_id: support-policy
    section_id: section-support-policy-008
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_7_exchanges
      action: requires
      label: Staff must waive the restocking fee described in Section 2 for exchanges
        of the same item in a different size or color, provided the request is made
        within the general return window described in Section 1.
    evidence_id: evidence-ki-support-policy-v1-000015
    relation_id: kr-ki-support-policy-v1-000015
    okf_path: rules/rule-ki-support-policy-v1-000015-requires.md
  knowledge_piece: ../knowledge/ki-support-policy-v1-000015.md
  knowledge_relation: ../relations/kr-ki-support-policy-v1-000015.md
  evidence: ../evidence/evidence-ki-support-policy-v1-000015.md
---
# 7. Exchanges

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_7_exchanges`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000015](../knowledge/ki-support-policy-v1-000015.md)
- Knowledge relation: [kr-ki-support-policy-v1-000015](../relations/kr-ki-support-policy-v1-000015.md)
- Evidence: [evidence-ki-support-policy-v1-000015](../evidence/evidence-ki-support-policy-v1-000015.md)
