---
type: SOP Decision Rule
title: Denial Routing
description: Staff must record appeal deadlines in the case note.
resource: ../knowledge/ki-prior-authorization-workflow-b5ce0527cf18-000013.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: '2026-08-10'
sources:
- id: src-prior-authorization-workflow-b5ce0527cf18
  title: prior-authorization-workflow-b5ce0527cf18
  resource: ../sources/prior-authorization-workflow-b5ce0527cf18.md
sopkb:
  rule:
    id: rule-ki-prior-authorization-workflow-b5ce0527cf18-000013-records
    type: SOP Decision Rule
    title: Denial Routing
    knowledge_item_id: ki-prior-authorization-workflow-b5ce0527cf18-000013
    source_id: prior-authorization-workflow-b5ce0527cf18
    section_id: section-prior-authorization-workflow-b5ce0527cf18-003
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_denial_routing
      action: records
      label: Staff must record appeal deadlines in the case note.
    evidence_id: evidence-ki-prior-authorization-workflow-b5ce0527cf18-000013
    relation_id: kr-ki-prior-authorization-workflow-b5ce0527cf18-000013
    okf_path: rules/rule-ki-prior-authorization-workflow-b5ce0527cf18-000013-records.md
  knowledge_piece: ../knowledge/ki-prior-authorization-workflow-b5ce0527cf18-000013.md
  knowledge_relation: ../relations/kr-ki-prior-authorization-workflow-b5ce0527cf18-000013.md
  evidence: ../evidence/evidence-ki-prior-authorization-workflow-b5ce0527cf18-000013.md
---
# Denial Routing

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_denial_routing`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-prior-authorization-workflow-b5ce0527cf18-000013](../knowledge/ki-prior-authorization-workflow-b5ce0527cf18-000013.md)
- Knowledge relation: [kr-ki-prior-authorization-workflow-b5ce0527cf18-000013](../relations/kr-ki-prior-authorization-workflow-b5ce0527cf18-000013.md)
- Evidence: [evidence-ki-prior-authorization-workflow-b5ce0527cf18-000013](../evidence/evidence-ki-prior-authorization-workflow-b5ce0527cf18-000013.md)
