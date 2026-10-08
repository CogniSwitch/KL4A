---
type: SOP Decision Rule
title: Requirements
description: Staff must confirm the checklist before closing the case.
resource: ../knowledge/ki-missing-dirs-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-missing-dirs
  title: missing dirs
  resource: ../sources/missing-dirs.md
sopkb:
  rule:
    id: rule-ki-missing-dirs-v1-000001-requires
    type: SOP Decision Rule
    title: Requirements
    knowledge_item_id: ki-missing-dirs-v1-000001
    source_id: missing-dirs
    section_id: section-missing-dirs-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_requirements
      action: requires
      label: Staff must confirm the checklist before closing the case.
    evidence_id: evidence-ki-missing-dirs-v1-000001
    relation_id: kr-ki-missing-dirs-v1-000001
    okf_path: rules/rule-ki-missing-dirs-v1-000001-requires.md
  knowledge_piece: ../knowledge/ki-missing-dirs-v1-000001.md
  knowledge_relation: ../relations/kr-ki-missing-dirs-v1-000001.md
  evidence: ../evidence/evidence-ki-missing-dirs-v1-000001.md
---
# Requirements

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_requirements`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-missing-dirs-v1-000001](../knowledge/ki-missing-dirs-v1-000001.md)
- Knowledge relation: [kr-ki-missing-dirs-v1-000001](../relations/kr-ki-missing-dirs-v1-000001.md)
- Evidence: [evidence-ki-missing-dirs-v1-000001](../evidence/evidence-ki-missing-dirs-v1-000001.md)
