---
type: SOP Decision Rule
title: Page 1
description: Staff must record dose tolerance, adverse event symptoms, and patient-reported
  adherence during follow-up.
resource: ../knowledge/ki-follow-up-monitoring-procedure-e25874854706-000002.md
tags:
- decision-rule
- edited
status: stable
generated:
  actor: sopkb/0.1.0
  date: '2026-08-10'
sources:
- id: src-follow-up-monitoring-procedure-e25874854706
  title: follow-up-monitoring-procedure-e25874854706
  resource: ../sources/follow-up-monitoring-procedure-e25874854706.md
sopkb:
  rule:
    id: rule-ki-follow-up-monitoring-procedure-e25874854706-000002-records
    type: SOP Decision Rule
    title: Page 1
    knowledge_item_id: ki-follow-up-monitoring-procedure-e25874854706-000002
    source_id: follow-up-monitoring-procedure-e25874854706
    section_id: section-follow-up-monitoring-procedure-e25874854706-002
    review_status: edited
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_page_1
      action: records
      label: Staff must record dose tolerance, adverse event symptoms, and patient-reported
        adherence during follow-up.
    evidence_id: evidence-ki-follow-up-monitoring-procedure-e25874854706-000002
    relation_id: kr-ki-follow-up-monitoring-procedure-e25874854706-000002
    okf_path: rules/rule-ki-follow-up-monitoring-procedure-e25874854706-000002-records.md
  knowledge_piece: ../knowledge/ki-follow-up-monitoring-procedure-e25874854706-000002.md
  knowledge_relation: ../relations/kr-ki-follow-up-monitoring-procedure-e25874854706-000002.md
  evidence: ../evidence/evidence-ki-follow-up-monitoring-procedure-e25874854706-000002.md
---
# Page 1

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_page_1`
- Review status: `edited`

## Connected Knowledge

- Knowledge piece: [ki-follow-up-monitoring-procedure-e25874854706-000002](../knowledge/ki-follow-up-monitoring-procedure-e25874854706-000002.md)
- Knowledge relation: [kr-ki-follow-up-monitoring-procedure-e25874854706-000002](../relations/kr-ki-follow-up-monitoring-procedure-e25874854706-000002.md)
- Evidence: [evidence-ki-follow-up-monitoring-procedure-e25874854706-000002](../evidence/evidence-ki-follow-up-monitoring-procedure-e25874854706-000002.md)
