---
type: SOP Decision Rule
title: Page 1
description: Clinicians must confirm patient tolerance before dose escalation.
resource: ../knowledge/ki-follow-up-monitoring-procedure-e25874854706-000003.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: '2026-08-10'
sources:
- id: src-follow-up-monitoring-procedure-e25874854706
  title: follow-up-monitoring-procedure-e25874854706
  resource: ../sources/follow-up-monitoring-procedure-e25874854706.md
sopkb:
  rule:
    id: rule-ki-follow-up-monitoring-procedure-e25874854706-000003-requires
    type: SOP Decision Rule
    title: Page 1
    knowledge_item_id: ki-follow-up-monitoring-procedure-e25874854706-000003
    source_id: follow-up-monitoring-procedure-e25874854706
    section_id: section-follow-up-monitoring-procedure-e25874854706-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_page_1
      action: requires
      label: Clinicians must confirm patient tolerance before dose escalation.
    evidence_id: evidence-ki-follow-up-monitoring-procedure-e25874854706-000003
    relation_id: kr-ki-follow-up-monitoring-procedure-e25874854706-000003
    okf_path: rules/rule-ki-follow-up-monitoring-procedure-e25874854706-000003-requires.md
  knowledge_piece: ../knowledge/ki-follow-up-monitoring-procedure-e25874854706-000003.md
  knowledge_relation: ../relations/kr-ki-follow-up-monitoring-procedure-e25874854706-000003.md
  evidence: ../evidence/evidence-ki-follow-up-monitoring-procedure-e25874854706-000003.md
---
# Page 1

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_page_1`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-follow-up-monitoring-procedure-e25874854706-000003](../knowledge/ki-follow-up-monitoring-procedure-e25874854706-000003.md)
- Knowledge relation: [kr-ki-follow-up-monitoring-procedure-e25874854706-000003](../relations/kr-ki-follow-up-monitoring-procedure-e25874854706-000003.md)
- Evidence: [evidence-ki-follow-up-monitoring-procedure-e25874854706-000003](../evidence/evidence-ki-follow-up-monitoring-procedure-e25874854706-000003.md)
