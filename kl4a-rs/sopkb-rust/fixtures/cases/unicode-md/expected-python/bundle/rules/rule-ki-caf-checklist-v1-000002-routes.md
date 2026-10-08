---
type: SOP Decision Rule
title: "Proc\xE9dure"
description: Le personnel doit record les contre-indications et route les cas incertains.
resource: ../knowledge/ki-caf-checklist-v1-000002.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-caf-checklist
  title: "caf\xE9 checklist"
  resource: ../sources/caf-checklist.md
sopkb:
  rule:
    id: rule-ki-caf-checklist-v1-000002-routes
    type: SOP Decision Rule
    title: "Proc\xE9dure"
    knowledge_item_id: ki-caf-checklist-v1-000002
    source_id: caf-checklist
    section_id: section-caf-checklist-003
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_proc_dure
      action: routes
      label: Le personnel doit record les contre-indications et route les cas incertains.
    evidence_id: evidence-ki-caf-checklist-v1-000002
    relation_id: kr-ki-caf-checklist-v1-000002
    okf_path: rules/rule-ki-caf-checklist-v1-000002-routes.md
  knowledge_piece: ../knowledge/ki-caf-checklist-v1-000002.md
  knowledge_relation: ../relations/kr-ki-caf-checklist-v1-000002.md
  evidence: ../evidence/evidence-ki-caf-checklist-v1-000002.md
---
# Procédure

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_proc_dure`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-caf-checklist-v1-000002](../knowledge/ki-caf-checklist-v1-000002.md)
- Knowledge relation: [kr-ki-caf-checklist-v1-000002](../relations/kr-ki-caf-checklist-v1-000002.md)
- Evidence: [evidence-ki-caf-checklist-v1-000002](../evidence/evidence-ki-caf-checklist-v1-000002.md)
