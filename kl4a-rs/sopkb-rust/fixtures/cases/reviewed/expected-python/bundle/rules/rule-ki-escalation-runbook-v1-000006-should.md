---
type: SOP Decision Rule
title: Post-Incident Review
description: The safety committee should record findings within five business days.
resource: ../knowledge/ki-escalation-runbook-v1-000006.md
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
    id: rule-ki-escalation-runbook-v1-000006-should
    type: SOP Decision Rule
    title: Post-Incident Review
    knowledge_item_id: ki-escalation-runbook-v1-000006
    source_id: escalation-runbook
    section_id: section-escalation-runbook-007
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_post_incident_review
      action: should
      label: The safety committee should record findings within five business days.
    evidence_id: evidence-ki-escalation-runbook-v1-000006
    relation_id: kr-ki-escalation-runbook-v1-000006
    okf_path: rules/rule-ki-escalation-runbook-v1-000006-should.md
  knowledge_piece: ../knowledge/ki-escalation-runbook-v1-000006.md
  knowledge_relation: ../relations/kr-ki-escalation-runbook-v1-000006.md
  evidence: ../evidence/evidence-ki-escalation-runbook-v1-000006.md
---
# Post-Incident Review

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_post_incident_review`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-escalation-runbook-v1-000006](../knowledge/ki-escalation-runbook-v1-000006.md)
- Knowledge relation: [kr-ki-escalation-runbook-v1-000006](../relations/kr-ki-escalation-runbook-v1-000006.md)
- Evidence: [evidence-ki-escalation-runbook-v1-000006](../evidence/evidence-ki-escalation-runbook-v1-000006.md)
