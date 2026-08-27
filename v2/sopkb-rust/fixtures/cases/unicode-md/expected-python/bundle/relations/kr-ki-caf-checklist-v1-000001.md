---
type: SOP Knowledge Relation
title: kr-ki-caf-checklist-v1-000001
description: "\xC9ligibilit\xE9 requires Le clinicien doit confirmer l'identit\xE9\
  \ du patient.."
resource: ../knowledge/ki-caf-checklist-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-caf-checklist
  title: "caf\xE9 checklist"
  resource: ../sources/caf-checklist.md
sopkb:
  relation:
    id: kr-ki-caf-checklist-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-ligibilit
      label: "\xC9ligibilit\xE9"
      text: "\xC9ligibilit\xE9"
      okf_path: concepts/concept-ligibilit.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-le-clinicien-doit-confirmer-l-identit-du-patient
      text: "Le clinicien doit confirmer l'identit\xE9 du patient."
      label: "Le clinicien doit confirmer l'identit\xE9 du patient."
    knowledge_piece_id: ki-caf-checklist-v1-000001
    evidence_id: evidence-ki-caf-checklist-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-caf-checklist-v1-000001

## Assertion

- Subject: [Éligibilité](../concepts/concept-ligibilit.md)
- Predicate: `requires`
- Object: Le clinicien doit confirmer l'identité du patient.

## Connected Knowledge

- Knowledge piece: [ki-caf-checklist-v1-000001](../knowledge/ki-caf-checklist-v1-000001.md)
- Evidence: [evidence-ki-caf-checklist-v1-000001](../evidence/evidence-ki-caf-checklist-v1-000001.md)
- Decision rule: [Éligibilité](../rules/rule-ki-caf-checklist-v1-000001-requires.md)
