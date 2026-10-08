---
type: SOP Decision Rule
title: Initial Notification
description: On-call staff must confirm receipt of the alert within 15 minutes.
resource: ../knowledge/ki-escalation-runbook-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-escalation-runbook
  title: escalation runbook
  resource: ../sources/escalation-runbook.md
sopkb:
  rule:
    id: rule-ki-escalation-runbook-v1-000001-requires
    type: SOP Decision Rule
    title: Initial Notification
    knowledge_item_id: ki-escalation-runbook-v1-000001
    source_id: escalation-runbook
    section_id: section-escalation-runbook-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_initial_notification
      action: requires
      label: On-call staff must confirm receipt of the alert within 15 minutes.
    evidence_id: evidence-ki-escalation-runbook-v1-000001
    relation_id: kr-ki-escalation-runbook-v1-000001
    okf_path: rules/rule-ki-escalation-runbook-v1-000001-requires.md
  knowledge_piece: ../knowledge/ki-escalation-runbook-v1-000001.md
  knowledge_relation: ../relations/kr-ki-escalation-runbook-v1-000001.md
  evidence: ../evidence/evidence-ki-escalation-runbook-v1-000001.md
---
# Initial Notification

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_initial_notification`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-escalation-runbook-v1-000001](../knowledge/ki-escalation-runbook-v1-000001.md)
- Knowledge relation: [kr-ki-escalation-runbook-v1-000001](../relations/kr-ki-escalation-runbook-v1-000001.md)
- Evidence: [evidence-ki-escalation-runbook-v1-000001](../evidence/evidence-ki-escalation-runbook-v1-000001.md)
