---
type: SOP Decision Rule
title: Escalation Procedure
description: Clinicians should route urgent cases to the on-call physician immediately.
resource: ../knowledge/ki-safety-crlf-v1-000002.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-safety-crlf
  title: safety crlf
  resource: ../sources/safety-crlf.md
sopkb:
  rule:
    id: rule-ki-safety-crlf-v1-000002-should
    type: SOP Decision Rule
    title: Escalation Procedure
    knowledge_item_id: ki-safety-crlf-v1-000002
    source_id: safety-crlf
    section_id: section-safety-crlf-003
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_escalation_procedure
      action: should
      label: Clinicians should route urgent cases to the on-call physician immediately.
    evidence_id: evidence-ki-safety-crlf-v1-000002
    relation_id: kr-ki-safety-crlf-v1-000002
    okf_path: rules/rule-ki-safety-crlf-v1-000002-should.md
  knowledge_piece: ../knowledge/ki-safety-crlf-v1-000002.md
  knowledge_relation: ../relations/kr-ki-safety-crlf-v1-000002.md
  evidence: ../evidence/evidence-ki-safety-crlf-v1-000002.md
---
# Escalation Procedure

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_escalation_procedure`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-safety-crlf-v1-000002](../knowledge/ki-safety-crlf-v1-000002.md)
- Knowledge relation: [kr-ki-safety-crlf-v1-000002](../relations/kr-ki-safety-crlf-v1-000002.md)
- Evidence: [evidence-ki-safety-crlf-v1-000002](../evidence/evidence-ki-safety-crlf-v1-000002.md)
