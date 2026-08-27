---
type: SOP Decision Rule
title: Vendor Escalation
description: The vendor liaison must record every escalation in the incident log.
resource: ../knowledge/ki-escalation-runbook-v1-000003.md
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
    id: rule-ki-escalation-runbook-v1-000003-records
    type: SOP Decision Rule
    title: Vendor Escalation
    knowledge_item_id: ki-escalation-runbook-v1-000003
    source_id: escalation-runbook
    section_id: section-escalation-runbook-004
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_vendor_escalation
      action: records
      label: The vendor liaison must record every escalation in the incident log.
    evidence_id: evidence-ki-escalation-runbook-v1-000003
    relation_id: kr-ki-escalation-runbook-v1-000003
    okf_path: rules/rule-ki-escalation-runbook-v1-000003-records.md
  knowledge_piece: ../knowledge/ki-escalation-runbook-v1-000003.md
  knowledge_relation: ../relations/kr-ki-escalation-runbook-v1-000003.md
  evidence: ../evidence/evidence-ki-escalation-runbook-v1-000003.md
---
# Vendor Escalation

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_vendor_escalation`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-escalation-runbook-v1-000003](../knowledge/ki-escalation-runbook-v1-000003.md)
- Knowledge relation: [kr-ki-escalation-runbook-v1-000003](../relations/kr-ki-escalation-runbook-v1-000003.md)
- Evidence: [evidence-ki-escalation-runbook-v1-000003](../evidence/evidence-ki-escalation-runbook-v1-000003.md)
