---
type: SOP Decision Rule
title: Requirements
description: Staff must confirm the manual override policy before deviating from the
  standard SOP.
resource: ../knowledge/ki-authored-okf-source-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-authored-okf-source
  title: authored okf source
  resource: ../sources/authored-okf-source.md
sopkb:
  rule:
    id: rule-ki-authored-okf-source-v1-000001-requires
    type: SOP Decision Rule
    title: Requirements
    knowledge_item_id: ki-authored-okf-source-v1-000001
    source_id: authored-okf-source
    section_id: section-authored-okf-source-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_requirements
      action: requires
      label: Staff must confirm the manual override policy before deviating from the
        standard SOP.
    evidence_id: evidence-ki-authored-okf-source-v1-000001
    relation_id: kr-ki-authored-okf-source-v1-000001
    okf_path: rules/rule-ki-authored-okf-source-v1-000001-requires.md
  knowledge_piece: ../knowledge/ki-authored-okf-source-v1-000001.md
  knowledge_relation: ../relations/kr-ki-authored-okf-source-v1-000001.md
  evidence: ../evidence/evidence-ki-authored-okf-source-v1-000001.md
---
# Requirements

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_requirements`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-authored-okf-source-v1-000001](../knowledge/ki-authored-okf-source-v1-000001.md)
- Knowledge relation: [kr-ki-authored-okf-source-v1-000001](../relations/kr-ki-authored-okf-source-v1-000001.md)
- Evidence: [evidence-ki-authored-okf-source-v1-000001](../evidence/evidence-ki-authored-okf-source-v1-000001.md)
