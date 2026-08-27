---
type: SOP Decision Rule
title: Route uncertain cases for clinical review
description: When case is uncertain, route case for clinical review.
resource: ../knowledge/ki-primary-care-glp1-sop-v1-000008.md
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
    id: rule-ki-primary-care-glp1-sop-v1-000008-route-uncertain-case
    type: SOP Decision Rule
    title: Route uncertain cases for clinical review
    knowledge_item_id: ki-primary-care-glp1-sop-v1-000008
    source_id: primary-care-glp1-sop
    section_id: section-primary-care-glp1-sop-003
    review_status: proposed
    confidence: 0.82
    condition:
      fact: case_uncertainty
      operator: is_true
      label: Case is uncertain
    obligation:
      fact: route_clinical_review
      action: route
      label: Route case for clinical review
    evidence_id: evidence-ki-primary-care-glp1-sop-v1-000008
    relation_id: kr-ki-primary-care-glp1-sop-v1-000008
    okf_path: rules/rule-ki-primary-care-glp1-sop-v1-000008-route-uncertain-case.md
    otherwise:
      action_required: false
      label: Clinical review route is not required by this rule when the case is not
        uncertain.
  knowledge_piece: ../knowledge/ki-primary-care-glp1-sop-v1-000008.md
  knowledge_relation: ../relations/kr-ki-primary-care-glp1-sop-v1-000008.md
  evidence: ../evidence/evidence-ki-primary-care-glp1-sop-v1-000008.md
---
# Route uncertain cases for clinical review

## Rule

- Condition: `case_uncertainty` is true
- Obligation: `route_clinical_review`
- Otherwise: Clinical review route is not required by this rule when the case is not uncertain.
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-primary-care-glp1-sop-v1-000008](../knowledge/ki-primary-care-glp1-sop-v1-000008.md)
- Knowledge relation: [kr-ki-primary-care-glp1-sop-v1-000008](../relations/kr-ki-primary-care-glp1-sop-v1-000008.md)
- Evidence: [evidence-ki-primary-care-glp1-sop-v1-000008](../evidence/evidence-ki-primary-care-glp1-sop-v1-000008.md)
