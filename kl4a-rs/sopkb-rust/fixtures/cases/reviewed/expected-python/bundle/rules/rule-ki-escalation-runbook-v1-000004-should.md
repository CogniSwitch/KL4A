---
type: SOP Decision Rule
title: Legal Notification
description: Staff should confirm legal has been notified for any reportable event.
resource: ../knowledge/ki-escalation-runbook-v1-000004.md
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
    id: rule-ki-escalation-runbook-v1-000004-should
    type: SOP Decision Rule
    title: Legal Notification
    knowledge_item_id: ki-escalation-runbook-v1-000004
    source_id: escalation-runbook
    section_id: section-escalation-runbook-005
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_legal_notification
      action: should
      label: Staff should confirm legal has been notified for any reportable event.
    evidence_id: evidence-ki-escalation-runbook-v1-000004
    relation_id: kr-ki-escalation-runbook-v1-000004
    okf_path: rules/rule-ki-escalation-runbook-v1-000004-should.md
  knowledge_piece: ../knowledge/ki-escalation-runbook-v1-000004.md
  knowledge_relation: ../relations/kr-ki-escalation-runbook-v1-000004.md
  evidence: ../evidence/evidence-ki-escalation-runbook-v1-000004.md
---
# Legal Notification

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_legal_notification`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-escalation-runbook-v1-000004](../knowledge/ki-escalation-runbook-v1-000004.md)
- Knowledge relation: [kr-ki-escalation-runbook-v1-000004](../relations/kr-ki-escalation-runbook-v1-000004.md)
- Evidence: [evidence-ki-escalation-runbook-v1-000004](../evidence/evidence-ki-escalation-runbook-v1-000004.md)
