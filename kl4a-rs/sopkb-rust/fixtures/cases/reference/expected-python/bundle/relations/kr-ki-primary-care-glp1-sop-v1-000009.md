---
type: SOP Knowledge Relation
title: kr-ki-primary-care-glp1-sop-v1-000009
description: Follow-up Monitoring should Patients should receive follow-up contact
  within 30 days after.
resource: ../knowledge/ki-primary-care-glp1-sop-v1-000009.md
tags:
- relation
- rdf-compatible
- should
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-primary-care-glp1-sop
  title: primary care glp1 sop
  resource: ../sources/primary-care-glp1-sop.md
sopkb:
  relation:
    id: kr-ki-primary-care-glp1-sop-v1-000009
    type: Knowledge Relation
    subject:
      id: concept-follow-up-monitoring
      label: Follow-up Monitoring
      text: Follow-up Monitoring
      okf_path: concepts/concept-follow-up-monitoring.md
    predicate:
      id: predicate-should
      text: should
    object:
      id: object-patients-should-receive-follow-up-contact-within-30-days-after
      text: Patients should receive follow-up contact within 30 days after GLP-1 therapy
        initiation.
      label: Patients should receive follow-up contact within 30 days after
    knowledge_piece_id: ki-primary-care-glp1-sop-v1-000009
    evidence_id: evidence-ki-primary-care-glp1-sop-v1-000009
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-primary-care-glp1-sop-v1-000009

## Assertion

- Subject: [Follow-up Monitoring](../concepts/concept-follow-up-monitoring.md)
- Predicate: `should`
- Object: Patients should receive follow-up contact within 30 days after GLP-1 therapy initiation.

## Connected Knowledge

- Knowledge piece: [ki-primary-care-glp1-sop-v1-000009](../knowledge/ki-primary-care-glp1-sop-v1-000009.md)
- Evidence: [evidence-ki-primary-care-glp1-sop-v1-000009](../evidence/evidence-ki-primary-care-glp1-sop-v1-000009.md)
- Decision rule: [Follow-up Monitoring](../rules/rule-ki-primary-care-glp1-sop-v1-000009-should.md)
