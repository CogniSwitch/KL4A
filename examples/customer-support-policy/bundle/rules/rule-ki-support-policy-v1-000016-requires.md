---
type: SOP Decision Rule
title: 7. Exchanges
description: Exchanges for a different product entirely must be processed as a return
  followed by a new purchase.
resource: ../knowledge/ki-support-policy-v1-000016.md
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
    id: rule-ki-support-policy-v1-000016-requires
    type: SOP Decision Rule
    title: 7. Exchanges
    knowledge_item_id: ki-support-policy-v1-000016
    source_id: support-policy
    section_id: section-support-policy-008
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_7_exchanges
      action: requires
      label: Exchanges for a different product entirely must be processed as a return
        followed by a new purchase.
    evidence_id: evidence-ki-support-policy-v1-000016
    relation_id: kr-ki-support-policy-v1-000016
    okf_path: rules/rule-ki-support-policy-v1-000016-requires.md
  knowledge_piece: ../knowledge/ki-support-policy-v1-000016.md
  knowledge_relation: ../relations/kr-ki-support-policy-v1-000016.md
  evidence: ../evidence/evidence-ki-support-policy-v1-000016.md
---
# 7. Exchanges

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_7_exchanges`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000016](../knowledge/ki-support-policy-v1-000016.md)
- Knowledge relation: [kr-ki-support-policy-v1-000016](../relations/kr-ki-support-policy-v1-000016.md)
- Evidence: [evidence-ki-support-policy-v1-000016](../evidence/evidence-ki-support-policy-v1-000016.md)
