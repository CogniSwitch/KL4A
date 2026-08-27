---
type: SOP Knowledge Relation
title: kr-ki-quarterly-review-v1-000001
description: Closing Notes requires This paragraph appears after the table in the
  visual.
resource: ../knowledge/ki-quarterly-review-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-quarterly-review
  title: quarterly review
  resource: ../sources/quarterly-review.md
sopkb:
  relation:
    id: kr-ki-quarterly-review-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-closing-notes
      label: Closing Notes
      text: Closing Notes
      okf_path: concepts/concept-closing-notes.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-this-paragraph-appears-after-the-table-in-the-visual
      text: This paragraph appears after the table in the visual document, but the
        table must be relocated to the very end of the normalized output, after this
        paragraph.
      label: This paragraph appears after the table in the visual
    knowledge_piece_id: ki-quarterly-review-v1-000001
    evidence_id: evidence-ki-quarterly-review-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-quarterly-review-v1-000001

## Assertion

- Subject: [Closing Notes](../concepts/concept-closing-notes.md)
- Predicate: `requires`
- Object: This paragraph appears after the table in the visual document, but the table must be relocated to the very end of the normalized output, after this paragraph.

## Connected Knowledge

- Knowledge piece: [ki-quarterly-review-v1-000001](../knowledge/ki-quarterly-review-v1-000001.md)
- Evidence: [evidence-ki-quarterly-review-v1-000001](../evidence/evidence-ki-quarterly-review-v1-000001.md)
- Decision rule: [Closing Notes](../rules/rule-ki-quarterly-review-v1-000001-requires.md)
