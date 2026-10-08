---
type: SOP Decision Rule
title: 'Purpose ##'
description: Staff must confirm the escalation procedure described below.
resource: ../knowledge/ki-weird-headings-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-weird-headings
  title: weird headings
  resource: ../sources/weird-headings.md
sopkb:
  rule:
    id: rule-ki-weird-headings-v1-000001-requires
    type: SOP Decision Rule
    title: 'Purpose ##'
    knowledge_item_id: ki-weird-headings-v1-000001
    source_id: weird-headings
    section_id: section-weird-headings-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_purpose
      action: requires
      label: Staff must confirm the escalation procedure described below.
    evidence_id: evidence-ki-weird-headings-v1-000001
    relation_id: kr-ki-weird-headings-v1-000001
    okf_path: rules/rule-ki-weird-headings-v1-000001-requires.md
  knowledge_piece: ../knowledge/ki-weird-headings-v1-000001.md
  knowledge_relation: ../relations/kr-ki-weird-headings-v1-000001.md
  evidence: ../evidence/evidence-ki-weird-headings-v1-000001.md
---
# Purpose ##

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_purpose`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-weird-headings-v1-000001](../knowledge/ki-weird-headings-v1-000001.md)
- Knowledge relation: [kr-ki-weird-headings-v1-000001](../relations/kr-ki-weird-headings-v1-000001.md)
- Evidence: [evidence-ki-weird-headings-v1-000001](../evidence/evidence-ki-weird-headings-v1-000001.md)
