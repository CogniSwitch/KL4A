---
type: SOP Decision Rule
title: Closing Notes
description: This paragraph appears after the table in the visual document, but the
  table must be relocated to the very end of the normalized output, after this paragraph.
resource: ../knowledge/ki-quarterly-review-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-quarterly-review
  title: quarterly review
  resource: ../sources/quarterly-review.md
sopkb:
  rule:
    id: rule-ki-quarterly-review-v1-000001-requires
    type: SOP Decision Rule
    title: Closing Notes
    knowledge_item_id: ki-quarterly-review-v1-000001
    source_id: quarterly-review
    section_id: section-quarterly-review-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_closing_notes
      action: requires
      label: This paragraph appears after the table in the visual document, but the
        table must be relocated to the very end of the normalized output, after this
        paragraph.
    evidence_id: evidence-ki-quarterly-review-v1-000001
    relation_id: kr-ki-quarterly-review-v1-000001
    okf_path: rules/rule-ki-quarterly-review-v1-000001-requires.md
  knowledge_piece: ../knowledge/ki-quarterly-review-v1-000001.md
  knowledge_relation: ../relations/kr-ki-quarterly-review-v1-000001.md
  evidence: ../evidence/evidence-ki-quarterly-review-v1-000001.md
---
# Closing Notes

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_closing_notes`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-quarterly-review-v1-000001](../knowledge/ki-quarterly-review-v1-000001.md)
- Knowledge relation: [kr-ki-quarterly-review-v1-000001](../relations/kr-ki-quarterly-review-v1-000001.md)
- Evidence: [evidence-ki-quarterly-review-v1-000001](../evidence/evidence-ki-quarterly-review-v1-000001.md)
