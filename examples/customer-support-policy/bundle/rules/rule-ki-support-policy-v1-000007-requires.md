---
type: SOP Decision Rule
title: 3. Electronics Exceptions
description: Electronics must be returned within 14 days of delivery, regardless of
  the general return window described in Section 1.
resource: ../knowledge/ki-support-policy-v1-000007.md
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
    id: rule-ki-support-policy-v1-000007-requires
    type: SOP Decision Rule
    title: 3. Electronics Exceptions
    knowledge_item_id: ki-support-policy-v1-000007
    source_id: support-policy
    section_id: section-support-policy-004
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_3_electronics_exceptions
      action: requires
      label: Electronics must be returned within 14 days of delivery, regardless of
        the general return window described in Section 1.
    evidence_id: evidence-ki-support-policy-v1-000007
    relation_id: kr-ki-support-policy-v1-000007
    okf_path: rules/rule-ki-support-policy-v1-000007-requires.md
  knowledge_piece: ../knowledge/ki-support-policy-v1-000007.md
  knowledge_relation: ../relations/kr-ki-support-policy-v1-000007.md
  evidence: ../evidence/evidence-ki-support-policy-v1-000007.md
---
# 3. Electronics Exceptions

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_3_electronics_exceptions`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000007](../knowledge/ki-support-policy-v1-000007.md)
- Knowledge relation: [kr-ki-support-policy-v1-000007](../relations/kr-ki-support-policy-v1-000007.md)
- Evidence: [evidence-ki-support-policy-v1-000007](../evidence/evidence-ki-support-policy-v1-000007.md)
