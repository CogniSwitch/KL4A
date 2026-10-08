---
type: SOP Knowledge Relation
title: kr-ki-caf-checklist-v1-000002
description: "Proc\xE9dure routes Le personnel doit record les contre-indications\
  \ et route les."
resource: ../knowledge/ki-caf-checklist-v1-000002.md
tags:
- relation
- rdf-compatible
- routes
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
    id: kr-ki-caf-checklist-v1-000002
    type: Knowledge Relation
    subject:
      id: concept-proc-dure
      label: "Proc\xE9dure"
      text: "Proc\xE9dure"
      okf_path: concepts/concept-proc-dure.md
    predicate:
      id: predicate-routes
      text: routes
    object:
      id: object-le-personnel-doit-record-les-contre-indications-et-route-les
      text: Le personnel doit record les contre-indications et route les cas incertains.
      label: Le personnel doit record les contre-indications et route les
    knowledge_piece_id: ki-caf-checklist-v1-000002
    evidence_id: evidence-ki-caf-checklist-v1-000002
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-caf-checklist-v1-000002

## Assertion

- Subject: [Procédure](../concepts/concept-proc-dure.md)
- Predicate: `routes`
- Object: Le personnel doit record les contre-indications et route les cas incertains.

## Connected Knowledge

- Knowledge piece: [ki-caf-checklist-v1-000002](../knowledge/ki-caf-checklist-v1-000002.md)
- Evidence: [evidence-ki-caf-checklist-v1-000002](../evidence/evidence-ki-caf-checklist-v1-000002.md)
- Decision rule: [Procédure](../rules/rule-ki-caf-checklist-v1-000002-routes.md)
