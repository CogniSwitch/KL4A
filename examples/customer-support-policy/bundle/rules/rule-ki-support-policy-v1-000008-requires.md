---
type: SOP Decision Rule
title: 3. Electronics Exceptions
description: Opened software and digital license keys must be treated as final sale
  and must not be accepted for return under any circumstances.
resource: ../knowledge/ki-support-policy-v1-000008.md
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
    id: rule-ki-support-policy-v1-000008-requires
    type: SOP Decision Rule
    title: 3. Electronics Exceptions
    knowledge_item_id: ki-support-policy-v1-000008
    source_id: support-policy
    section_id: section-support-policy-004
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_3_electronics_exceptions
      action: requires
      label: Opened software and digital license keys must be treated as final sale
        and must not be accepted for return under any circumstances.
    evidence_id: evidence-ki-support-policy-v1-000008
    relation_id: kr-ki-support-policy-v1-000008
    okf_path: rules/rule-ki-support-policy-v1-000008-requires.md
  knowledge_piece: ../knowledge/ki-support-policy-v1-000008.md
  knowledge_relation: ../relations/kr-ki-support-policy-v1-000008.md
  evidence: ../evidence/evidence-ki-support-policy-v1-000008.md
---
# 3. Electronics Exceptions

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_3_electronics_exceptions`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000008](../knowledge/ki-support-policy-v1-000008.md)
- Knowledge relation: [kr-ki-support-policy-v1-000008](../relations/kr-ki-support-policy-v1-000008.md)
- Evidence: [evidence-ki-support-policy-v1-000008](../evidence/evidence-ki-support-policy-v1-000008.md)
