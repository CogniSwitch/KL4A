---
type: SOP Decision Rule
title: Contraindication Screening
description: Clinicians must screen for pregnancy, pancreatitis history, medullary
  thyroid carcinoma history, and multiple endocrine neoplasia type 2 history.
resource: ../knowledge/ki-primary-care-glp1-sop-v1-000007.md
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
    id: rule-ki-primary-care-glp1-sop-v1-000007-requires
    type: SOP Decision Rule
    title: Contraindication Screening
    knowledge_item_id: ki-primary-care-glp1-sop-v1-000007
    source_id: primary-care-glp1-sop
    section_id: section-primary-care-glp1-sop-003
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_contraindication_screening
      action: requires
      label: Clinicians must screen for pregnancy, pancreatitis history, medullary
        thyroid carcinoma history, and multiple endocrine neoplasia type 2 history.
    evidence_id: evidence-ki-primary-care-glp1-sop-v1-000007
    relation_id: kr-ki-primary-care-glp1-sop-v1-000007
    okf_path: rules/rule-ki-primary-care-glp1-sop-v1-000007-requires.md
  knowledge_piece: ../knowledge/ki-primary-care-glp1-sop-v1-000007.md
  knowledge_relation: ../relations/kr-ki-primary-care-glp1-sop-v1-000007.md
  evidence: ../evidence/evidence-ki-primary-care-glp1-sop-v1-000007.md
---
# Contraindication Screening

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_contraindication_screening`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-primary-care-glp1-sop-v1-000007](../knowledge/ki-primary-care-glp1-sop-v1-000007.md)
- Knowledge relation: [kr-ki-primary-care-glp1-sop-v1-000007](../relations/kr-ki-primary-care-glp1-sop-v1-000007.md)
- Evidence: [evidence-ki-primary-care-glp1-sop-v1-000007](../evidence/evidence-ki-primary-care-glp1-sop-v1-000007.md)
