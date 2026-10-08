---
type: SOP Decision Rule
title: not a real heading, inside a fence
description: Clinicians should route uncertain cases for review.
resource: ../knowledge/ki-weird-headings-v1-000002.md
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
    id: rule-ki-weird-headings-v1-000002-should
    type: SOP Decision Rule
    title: not a real heading, inside a fence
    knowledge_item_id: ki-weird-headings-v1-000002
    source_id: weird-headings
    section_id: section-weird-headings-003
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_not_a_real_heading_inside_a_fence
      action: should
      label: Clinicians should route uncertain cases for review.
    evidence_id: evidence-ki-weird-headings-v1-000002
    relation_id: kr-ki-weird-headings-v1-000002
    okf_path: rules/rule-ki-weird-headings-v1-000002-should.md
  knowledge_piece: ../knowledge/ki-weird-headings-v1-000002.md
  knowledge_relation: ../relations/kr-ki-weird-headings-v1-000002.md
  evidence: ../evidence/evidence-ki-weird-headings-v1-000002.md
---
# not a real heading, inside a fence

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_not_a_real_heading_inside_a_fence`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-weird-headings-v1-000002](../knowledge/ki-weird-headings-v1-000002.md)
- Knowledge relation: [kr-ki-weird-headings-v1-000002](../relations/kr-ki-weird-headings-v1-000002.md)
- Evidence: [evidence-ki-weird-headings-v1-000002](../evidence/evidence-ki-weird-headings-v1-000002.md)
