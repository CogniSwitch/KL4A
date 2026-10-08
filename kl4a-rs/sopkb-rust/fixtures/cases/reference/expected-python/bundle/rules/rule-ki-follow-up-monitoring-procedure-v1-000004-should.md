---
type: SOP Decision Rule
title: Dose Titration
description: Clinicians should defer dose escalation when severe gastrointestinal
  symptoms are reported.
resource: ../knowledge/ki-follow-up-monitoring-procedure-v1-000004.md
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
    id: rule-ki-follow-up-monitoring-procedure-v1-000004-should
    type: SOP Decision Rule
    title: Dose Titration
    knowledge_item_id: ki-follow-up-monitoring-procedure-v1-000004
    source_id: follow-up-monitoring-procedure
    section_id: section-follow-up-monitoring-procedure-003
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_dose_titration
      action: should
      label: Clinicians should defer dose escalation when severe gastrointestinal
        symptoms are reported.
    evidence_id: evidence-ki-follow-up-monitoring-procedure-v1-000004
    relation_id: kr-ki-follow-up-monitoring-procedure-v1-000004
    okf_path: rules/rule-ki-follow-up-monitoring-procedure-v1-000004-should.md
  knowledge_piece: ../knowledge/ki-follow-up-monitoring-procedure-v1-000004.md
  knowledge_relation: ../relations/kr-ki-follow-up-monitoring-procedure-v1-000004.md
  evidence: ../evidence/evidence-ki-follow-up-monitoring-procedure-v1-000004.md
---
# Dose Titration

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_dose_titration`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-follow-up-monitoring-procedure-v1-000004](../knowledge/ki-follow-up-monitoring-procedure-v1-000004.md)
- Knowledge relation: [kr-ki-follow-up-monitoring-procedure-v1-000004](../relations/kr-ki-follow-up-monitoring-procedure-v1-000004.md)
- Evidence: [evidence-ki-follow-up-monitoring-procedure-v1-000004](../evidence/evidence-ki-follow-up-monitoring-procedure-v1-000004.md)
