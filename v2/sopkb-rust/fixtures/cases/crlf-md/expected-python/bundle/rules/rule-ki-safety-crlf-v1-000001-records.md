---
type: SOP Decision Rule
title: Reporting Requirements
description: Staff must record all adverse events within 24 hours.
resource: ../knowledge/ki-safety-crlf-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-safety-crlf
  title: safety crlf
  resource: ../sources/safety-crlf.md
sopkb:
  rule:
    id: rule-ki-safety-crlf-v1-000001-records
    type: SOP Decision Rule
    title: Reporting Requirements
    knowledge_item_id: ki-safety-crlf-v1-000001
    source_id: safety-crlf
    section_id: section-safety-crlf-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_reporting_requirements
      action: records
      label: Staff must record all adverse events within 24 hours.
    evidence_id: evidence-ki-safety-crlf-v1-000001
    relation_id: kr-ki-safety-crlf-v1-000001
    okf_path: rules/rule-ki-safety-crlf-v1-000001-records.md
  knowledge_piece: ../knowledge/ki-safety-crlf-v1-000001.md
  knowledge_relation: ../relations/kr-ki-safety-crlf-v1-000001.md
  evidence: ../evidence/evidence-ki-safety-crlf-v1-000001.md
---
# Reporting Requirements

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_reporting_requirements`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-safety-crlf-v1-000001](../knowledge/ki-safety-crlf-v1-000001.md)
- Knowledge relation: [kr-ki-safety-crlf-v1-000001](../relations/kr-ki-safety-crlf-v1-000001.md)
- Evidence: [evidence-ki-safety-crlf-v1-000001](../evidence/evidence-ki-safety-crlf-v1-000001.md)
