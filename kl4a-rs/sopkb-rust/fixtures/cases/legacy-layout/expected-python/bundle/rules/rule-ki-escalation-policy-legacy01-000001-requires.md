---
type: SOP Decision Rule
title: Escalation Policy
description: On-call staff must confirm receipt of every critical alert within 15
  minutes.
resource: ../knowledge/ki-escalation-policy-legacy01-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-escalation-policy-legacy01
  title: Escalation Policy
  resource: ../sources/escalation-policy-legacy01.md
sopkb:
  rule:
    id: rule-ki-escalation-policy-legacy01-000001-requires
    type: SOP Decision Rule
    title: Escalation Policy
    knowledge_item_id: ki-escalation-policy-legacy01-000001
    source_id: escalation-policy-legacy01
    section_id: section-escalation-policy-legacy01-001
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_escalation_policy
      action: requires
      label: On-call staff must confirm receipt of every critical alert within 15
        minutes.
    evidence_id: evidence-ki-escalation-policy-legacy01-000001
    relation_id: kr-ki-escalation-policy-legacy01-000001
    okf_path: rules/rule-ki-escalation-policy-legacy01-000001-requires.md
  knowledge_piece: ../knowledge/ki-escalation-policy-legacy01-000001.md
  knowledge_relation: ../relations/kr-ki-escalation-policy-legacy01-000001.md
  evidence: ../evidence/evidence-ki-escalation-policy-legacy01-000001.md
---
# Escalation Policy

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_escalation_policy`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-escalation-policy-legacy01-000001](../knowledge/ki-escalation-policy-legacy01-000001.md)
- Knowledge relation: [kr-ki-escalation-policy-legacy01-000001](../relations/kr-ki-escalation-policy-legacy01-000001.md)
- Evidence: [evidence-ki-escalation-policy-legacy01-000001](../evidence/evidence-ki-escalation-policy-legacy01-000001.md)
