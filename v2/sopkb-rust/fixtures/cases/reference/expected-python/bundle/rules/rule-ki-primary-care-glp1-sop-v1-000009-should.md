---
type: SOP Decision Rule
title: Follow-up Monitoring
description: Patients should receive follow-up contact within 30 days after GLP-1
  therapy initiation.
resource: ../knowledge/ki-primary-care-glp1-sop-v1-000009.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-primary-care-glp1-sop
  title: primary care glp1 sop
  resource: ../sources/primary-care-glp1-sop.md
sopkb:
  rule:
    id: rule-ki-primary-care-glp1-sop-v1-000009-should
    type: SOP Decision Rule
    title: Follow-up Monitoring
    knowledge_item_id: ki-primary-care-glp1-sop-v1-000009
    source_id: primary-care-glp1-sop
    section_id: section-primary-care-glp1-sop-004
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_follow_up_monitoring
      action: should
      label: Patients should receive follow-up contact within 30 days after GLP-1
        therapy initiation.
    evidence_id: evidence-ki-primary-care-glp1-sop-v1-000009
    relation_id: kr-ki-primary-care-glp1-sop-v1-000009
    okf_path: rules/rule-ki-primary-care-glp1-sop-v1-000009-should.md
  knowledge_piece: ../knowledge/ki-primary-care-glp1-sop-v1-000009.md
  knowledge_relation: ../relations/kr-ki-primary-care-glp1-sop-v1-000009.md
  evidence: ../evidence/evidence-ki-primary-care-glp1-sop-v1-000009.md
---
# Follow-up Monitoring

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_follow_up_monitoring`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-primary-care-glp1-sop-v1-000009](../knowledge/ki-primary-care-glp1-sop-v1-000009.md)
- Knowledge relation: [kr-ki-primary-care-glp1-sop-v1-000009](../relations/kr-ki-primary-care-glp1-sop-v1-000009.md)
- Evidence: [evidence-ki-primary-care-glp1-sop-v1-000009](../evidence/evidence-ki-primary-care-glp1-sop-v1-000009.md)
