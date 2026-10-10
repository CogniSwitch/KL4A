---
type: SOP Decision Rule
title: 8. Escalation and Manager Approval
description: Returns requested outside any window described above must receive manager
  approval before processing.
resource: ../knowledge/ki-support-policy-v1-000017.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sources:
- id: src-support-policy
  title: support policy
  resource: ../sources/support-policy.md
sopkb:
  rule:
    id: rule-ki-support-policy-v1-000017-requires
    type: SOP Decision Rule
    title: 8. Escalation and Manager Approval
    knowledge_item_id: ki-support-policy-v1-000017
    source_id: support-policy
    section_id: section-support-policy-009
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_8_escalation_and_manager_approval
      action: requires
      label: Returns requested outside any window described above must receive manager
        approval before processing.
    evidence_id: evidence-ki-support-policy-v1-000017
    relation_id: kr-ki-support-policy-v1-000017
    okf_path: rules/rule-ki-support-policy-v1-000017-requires.md
  knowledge_piece: ../knowledge/ki-support-policy-v1-000017.md
  knowledge_relation: ../relations/kr-ki-support-policy-v1-000017.md
  evidence: ../evidence/evidence-ki-support-policy-v1-000017.md
---
# 8. Escalation and Manager Approval

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_8_escalation_and_manager_approval`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000017](../knowledge/ki-support-policy-v1-000017.md)
- Knowledge relation: [kr-ki-support-policy-v1-000017](../relations/kr-ki-support-policy-v1-000017.md)
- Evidence: [evidence-ki-support-policy-v1-000017](../evidence/evidence-ki-support-policy-v1-000017.md)
