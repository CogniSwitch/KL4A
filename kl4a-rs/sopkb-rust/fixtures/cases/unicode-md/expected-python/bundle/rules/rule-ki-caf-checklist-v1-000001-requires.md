---
type: SOP Decision Rule
title: "\xC9ligibilit\xE9"
description: "Le clinicien doit confirmer l'identit\xE9 du patient."
resource: ../knowledge/ki-caf-checklist-v1-000001.md
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
    id: rule-ki-caf-checklist-v1-000001-requires
    type: SOP Decision Rule
    title: "\xC9ligibilit\xE9"
    knowledge_item_id: ki-caf-checklist-v1-000001
    source_id: caf-checklist
    section_id: section-caf-checklist-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_ligibilit
      action: requires
      label: "Le clinicien doit confirmer l'identit\xE9 du patient."
    evidence_id: evidence-ki-caf-checklist-v1-000001
    relation_id: kr-ki-caf-checklist-v1-000001
    okf_path: rules/rule-ki-caf-checklist-v1-000001-requires.md
  knowledge_piece: ../knowledge/ki-caf-checklist-v1-000001.md
  knowledge_relation: ../relations/kr-ki-caf-checklist-v1-000001.md
  evidence: ../evidence/evidence-ki-caf-checklist-v1-000001.md
---
# Éligibilité

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_ligibilit`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-caf-checklist-v1-000001](../knowledge/ki-caf-checklist-v1-000001.md)
- Knowledge relation: [kr-ki-caf-checklist-v1-000001](../relations/kr-ki-caf-checklist-v1-000001.md)
- Evidence: [evidence-ki-caf-checklist-v1-000001](../evidence/evidence-ki-caf-checklist-v1-000001.md)
