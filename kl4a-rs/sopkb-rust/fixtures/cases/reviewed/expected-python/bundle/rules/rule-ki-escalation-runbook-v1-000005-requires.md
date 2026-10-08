---
type: SOP Decision Rule
title: Patient Communication
description: Care coordinators must confirm the patient has been informed of the outcome.
resource: ../knowledge/ki-escalation-runbook-v1-000005.md
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
    id: rule-ki-escalation-runbook-v1-000005-requires
    type: SOP Decision Rule
    title: Patient Communication
    knowledge_item_id: ki-escalation-runbook-v1-000005
    source_id: escalation-runbook
    section_id: section-escalation-runbook-006
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_patient_communication
      action: requires
      label: Care coordinators must confirm the patient has been informed of the outcome.
    evidence_id: evidence-ki-escalation-runbook-v1-000005
    relation_id: kr-ki-escalation-runbook-v1-000005
    okf_path: rules/rule-ki-escalation-runbook-v1-000005-requires.md
  knowledge_piece: ../knowledge/ki-escalation-runbook-v1-000005.md
  knowledge_relation: ../relations/kr-ki-escalation-runbook-v1-000005.md
  evidence: ../evidence/evidence-ki-escalation-runbook-v1-000005.md
---
# Patient Communication

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_patient_communication`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-escalation-runbook-v1-000005](../knowledge/ki-escalation-runbook-v1-000005.md)
- Knowledge relation: [kr-ki-escalation-runbook-v1-000005](../relations/kr-ki-escalation-runbook-v1-000005.md)
- Evidence: [evidence-ki-escalation-runbook-v1-000005](../evidence/evidence-ki-escalation-runbook-v1-000005.md)
