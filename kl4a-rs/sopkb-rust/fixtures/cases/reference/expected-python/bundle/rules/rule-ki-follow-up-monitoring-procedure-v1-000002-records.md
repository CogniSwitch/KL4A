---
type: SOP Decision Rule
title: Follow-up Monitoring
description: Staff must record dose tolerance and adverse event symptoms during follow-up.
resource: ../knowledge/ki-follow-up-monitoring-procedure-v1-000002.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-follow-up-monitoring-procedure
  title: follow up monitoring procedure
  resource: ../sources/follow-up-monitoring-procedure.md
sopkb:
  rule:
    id: rule-ki-follow-up-monitoring-procedure-v1-000002-records
    type: SOP Decision Rule
    title: Follow-up Monitoring
    knowledge_item_id: ki-follow-up-monitoring-procedure-v1-000002
    source_id: follow-up-monitoring-procedure
    section_id: section-follow-up-monitoring-procedure-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_follow_up_monitoring
      action: records
      label: Staff must record dose tolerance and adverse event symptoms during follow-up.
    evidence_id: evidence-ki-follow-up-monitoring-procedure-v1-000002
    relation_id: kr-ki-follow-up-monitoring-procedure-v1-000002
    okf_path: rules/rule-ki-follow-up-monitoring-procedure-v1-000002-records.md
  knowledge_piece: ../knowledge/ki-follow-up-monitoring-procedure-v1-000002.md
  knowledge_relation: ../relations/kr-ki-follow-up-monitoring-procedure-v1-000002.md
  evidence: ../evidence/evidence-ki-follow-up-monitoring-procedure-v1-000002.md
---
# Follow-up Monitoring

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_follow_up_monitoring`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-follow-up-monitoring-procedure-v1-000002](../knowledge/ki-follow-up-monitoring-procedure-v1-000002.md)
- Knowledge relation: [kr-ki-follow-up-monitoring-procedure-v1-000002](../relations/kr-ki-follow-up-monitoring-procedure-v1-000002.md)
- Evidence: [evidence-ki-follow-up-monitoring-procedure-v1-000002](../evidence/evidence-ki-follow-up-monitoring-procedure-v1-000002.md)
