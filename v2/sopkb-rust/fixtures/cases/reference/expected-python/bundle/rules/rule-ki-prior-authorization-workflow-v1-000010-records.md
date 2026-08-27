---
type: SOP Decision Rule
title: Prior Authorization Requirements
description: Staff must record payer requirements before submitting a GLP-1 prior
  authorization request.
resource: ../knowledge/ki-prior-authorization-workflow-v1-000010.md
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
    id: rule-ki-prior-authorization-workflow-v1-000010-records
    type: SOP Decision Rule
    title: Prior Authorization Requirements
    knowledge_item_id: ki-prior-authorization-workflow-v1-000010
    source_id: prior-authorization-workflow
    section_id: section-prior-authorization-workflow-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_prior_authorization_requirements
      action: records
      label: Staff must record payer requirements before submitting a GLP-1 prior
        authorization request.
    evidence_id: evidence-ki-prior-authorization-workflow-v1-000010
    relation_id: kr-ki-prior-authorization-workflow-v1-000010
    okf_path: rules/rule-ki-prior-authorization-workflow-v1-000010-records.md
  knowledge_piece: ../knowledge/ki-prior-authorization-workflow-v1-000010.md
  knowledge_relation: ../relations/kr-ki-prior-authorization-workflow-v1-000010.md
  evidence: ../evidence/evidence-ki-prior-authorization-workflow-v1-000010.md
---
# Prior Authorization Requirements

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_prior_authorization_requirements`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-prior-authorization-workflow-v1-000010](../knowledge/ki-prior-authorization-workflow-v1-000010.md)
- Knowledge relation: [kr-ki-prior-authorization-workflow-v1-000010](../relations/kr-ki-prior-authorization-workflow-v1-000010.md)
- Evidence: [evidence-ki-prior-authorization-workflow-v1-000010](../evidence/evidence-ki-prior-authorization-workflow-v1-000010.md)
