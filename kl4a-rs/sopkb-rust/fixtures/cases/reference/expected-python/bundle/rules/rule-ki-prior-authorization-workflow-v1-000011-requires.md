---
type: SOP Decision Rule
title: Prior Authorization Requirements
description: Staff must attach diagnosis evidence and prior therapy documentation
  when required by the payer.
resource: ../knowledge/ki-prior-authorization-workflow-v1-000011.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-prior-authorization-workflow
  title: prior authorization workflow
  resource: ../sources/prior-authorization-workflow.md
sopkb:
  rule:
    id: rule-ki-prior-authorization-workflow-v1-000011-requires
    type: SOP Decision Rule
    title: Prior Authorization Requirements
    knowledge_item_id: ki-prior-authorization-workflow-v1-000011
    source_id: prior-authorization-workflow
    section_id: section-prior-authorization-workflow-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_prior_authorization_requirements
      action: requires
      label: Staff must attach diagnosis evidence and prior therapy documentation
        when required by the payer.
    evidence_id: evidence-ki-prior-authorization-workflow-v1-000011
    relation_id: kr-ki-prior-authorization-workflow-v1-000011
    okf_path: rules/rule-ki-prior-authorization-workflow-v1-000011-requires.md
  knowledge_piece: ../knowledge/ki-prior-authorization-workflow-v1-000011.md
  knowledge_relation: ../relations/kr-ki-prior-authorization-workflow-v1-000011.md
  evidence: ../evidence/evidence-ki-prior-authorization-workflow-v1-000011.md
---
# Prior Authorization Requirements

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_prior_authorization_requirements`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-prior-authorization-workflow-v1-000011](../knowledge/ki-prior-authorization-workflow-v1-000011.md)
- Knowledge relation: [kr-ki-prior-authorization-workflow-v1-000011](../relations/kr-ki-prior-authorization-workflow-v1-000011.md)
- Evidence: [evidence-ki-prior-authorization-workflow-v1-000011](../evidence/evidence-ki-prior-authorization-workflow-v1-000011.md)
