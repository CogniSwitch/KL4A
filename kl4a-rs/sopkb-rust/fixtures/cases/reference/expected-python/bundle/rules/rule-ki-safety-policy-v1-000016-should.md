---
type: SOP Decision Rule
title: Adverse Event Monitoring
description: Clinicians should record severe gastrointestinal symptoms, dehydration
  risk, and suspected pancreatitis symptoms.
resource: ../knowledge/ki-safety-policy-v1-000016.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-safety-policy
  title: safety policy
  resource: ../sources/safety-policy.md
sopkb:
  rule:
    id: rule-ki-safety-policy-v1-000016-should
    type: SOP Decision Rule
    title: Adverse Event Monitoring
    knowledge_item_id: ki-safety-policy-v1-000016
    source_id: safety-policy
    section_id: section-safety-policy-003
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_adverse_event_monitoring
      action: should
      label: Clinicians should record severe gastrointestinal symptoms, dehydration
        risk, and suspected pancreatitis symptoms.
    evidence_id: evidence-ki-safety-policy-v1-000016
    relation_id: kr-ki-safety-policy-v1-000016
    okf_path: rules/rule-ki-safety-policy-v1-000016-should.md
  knowledge_piece: ../knowledge/ki-safety-policy-v1-000016.md
  knowledge_relation: ../relations/kr-ki-safety-policy-v1-000016.md
  evidence: ../evidence/evidence-ki-safety-policy-v1-000016.md
---
# Adverse Event Monitoring

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_adverse_event_monitoring`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-safety-policy-v1-000016](../knowledge/ki-safety-policy-v1-000016.md)
- Knowledge relation: [kr-ki-safety-policy-v1-000016](../relations/kr-ki-safety-policy-v1-000016.md)
- Evidence: [evidence-ki-safety-policy-v1-000016](../evidence/evidence-ki-safety-policy-v1-000016.md)
