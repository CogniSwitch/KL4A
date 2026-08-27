---
type: SOP Decision Rule
title: Route uncertain cases for clinical review
description: When case is uncertain, route case for clinical review.
resource: ../knowledge/ki-glp1-intake-v1-000002.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-glp1-intake
  title: glp1 intake
  resource: ../sources/glp1-intake.md
sopkb:
  rule:
    id: rule-ki-glp1-intake-v1-000002-route-uncertain-case
    type: SOP Decision Rule
    title: Route uncertain cases for clinical review
    knowledge_item_id: ki-glp1-intake-v1-000002
    source_id: glp1-intake
    section_id: section-glp1-intake-004
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
    evidence_id: evidence-ki-glp1-intake-v1-000002
    relation_id: kr-ki-glp1-intake-v1-000002
    okf_path: rules/rule-ki-glp1-intake-v1-000002-route-uncertain-case.md
    otherwise:
      action_required: false
      label: Clinical review route is not required by this rule when the case is not
        uncertain.
  knowledge_piece: ../knowledge/ki-glp1-intake-v1-000002.md
  knowledge_relation: ../relations/kr-ki-glp1-intake-v1-000002.md
  evidence: ../evidence/evidence-ki-glp1-intake-v1-000002.md
---
# Route uncertain cases for clinical review

## Rule

- Condition: `case_uncertainty` is true
- Obligation: `route_clinical_review`
- Otherwise: Clinical review route is not required by this rule when the case is not uncertain.
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-glp1-intake-v1-000002](../knowledge/ki-glp1-intake-v1-000002.md)
- Knowledge relation: [kr-ki-glp1-intake-v1-000002](../relations/kr-ki-glp1-intake-v1-000002.md)
- Evidence: [evidence-ki-glp1-intake-v1-000002](../evidence/evidence-ki-glp1-intake-v1-000002.md)
