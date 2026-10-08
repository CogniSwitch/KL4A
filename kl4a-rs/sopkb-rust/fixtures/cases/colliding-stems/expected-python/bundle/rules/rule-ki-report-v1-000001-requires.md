---
type: SOP Decision Rule
title: Report
description: Staff must confirm the report contents before filing.
resource: ../knowledge/ki-report-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-report
  title: report
  resource: ../sources/report.md
sopkb:
  rule:
    id: rule-ki-report-v1-000001-requires
    type: SOP Decision Rule
    title: Report
    knowledge_item_id: ki-report-v1-000001
    source_id: report
    section_id: section-report-001
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_report
      action: requires
      label: Staff must confirm the report contents before filing.
    evidence_id: evidence-ki-report-v1-000001
    relation_id: kr-ki-report-v1-000001
    okf_path: rules/rule-ki-report-v1-000001-requires.md
  knowledge_piece: ../knowledge/ki-report-v1-000001.md
  knowledge_relation: ../relations/kr-ki-report-v1-000001.md
  evidence: ../evidence/evidence-ki-report-v1-000001.md
---
# Report

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_report`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-report-v1-000001](../knowledge/ki-report-v1-000001.md)
- Knowledge relation: [kr-ki-report-v1-000001](../relations/kr-ki-report-v1-000001.md)
- Evidence: [evidence-ki-report-v1-000001](../evidence/evidence-ki-report-v1-000001.md)
