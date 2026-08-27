---
type: SOP Decision Rule
title: Record contraindications
description: Contraindications recorded
resource: ../knowledge/ki-glp1-intake-v1-000002.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-glp1-intake
  title: glp1 intake
  resource: ../sources/glp1-intake.md
sopkb:
  rule:
    id: rule-ki-glp1-intake-v1-000002-record-contraindications
    type: SOP Decision Rule
    title: Record contraindications
    knowledge_item_id: ki-glp1-intake-v1-000002
    source_id: glp1-intake
    section_id: section-glp1-intake-004
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: contraindications_recorded
      action: record
      label: Contraindications recorded
    evidence_id: evidence-ki-glp1-intake-v1-000002
    relation_id: kr-ki-glp1-intake-v1-000002
    okf_path: rules/rule-ki-glp1-intake-v1-000002-record-contraindications.md
  knowledge_piece: ../knowledge/ki-glp1-intake-v1-000002.md
  knowledge_relation: ../relations/kr-ki-glp1-intake-v1-000002.md
  evidence: ../evidence/evidence-ki-glp1-intake-v1-000002.md
---
# Record contraindications

## Rule

- Condition: always applies
- Obligation: `contraindications_recorded`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-glp1-intake-v1-000002](../knowledge/ki-glp1-intake-v1-000002.md)
- Knowledge relation: [kr-ki-glp1-intake-v1-000002](../relations/kr-ki-glp1-intake-v1-000002.md)
- Evidence: [evidence-ki-glp1-intake-v1-000002](../evidence/evidence-ki-glp1-intake-v1-000002.md)
