---
type: SOP Decision Rule
title: 8. Escalation and Manager Approval
description: Staff must record manager-approved exceptions in the order notes before
  a refund or credit is issued.
resource: ../knowledge/ki-support-policy-v1-000018.md
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
    id: rule-ki-support-policy-v1-000018-records
    type: SOP Decision Rule
    title: 8. Escalation and Manager Approval
    knowledge_item_id: ki-support-policy-v1-000018
    source_id: support-policy
    section_id: section-support-policy-009
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_8_escalation_and_manager_approval
      action: records
      label: Staff must record manager-approved exceptions in the order notes before
        a refund or credit is issued.
    evidence_id: evidence-ki-support-policy-v1-000018
    relation_id: kr-ki-support-policy-v1-000018
    okf_path: rules/rule-ki-support-policy-v1-000018-records.md
  knowledge_piece: ../knowledge/ki-support-policy-v1-000018.md
  knowledge_relation: ../relations/kr-ki-support-policy-v1-000018.md
  evidence: ../evidence/evidence-ki-support-policy-v1-000018.md
---
# 8. Escalation and Manager Approval

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_8_escalation_and_manager_approval`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000018](../knowledge/ki-support-policy-v1-000018.md)
- Knowledge relation: [kr-ki-support-policy-v1-000018](../relations/kr-ki-support-policy-v1-000018.md)
- Evidence: [evidence-ki-support-policy-v1-000018](../evidence/evidence-ki-support-policy-v1-000018.md)
