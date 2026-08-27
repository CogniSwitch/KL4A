---
type: SOP Decision Rule
title: Contraindication Screening
description: Patients with confirmed contraindications must not start GLP-1 therapy
  without specialist review.
resource: ../knowledge/ki-safety-policy-01ff452b1d44-000015.md
tags:
- decision-rule
- deferred
status: draft
generated:
  actor: sopkb/0.1.0
  date: '2026-08-10'
sources:
- id: src-safety-policy-01ff452b1d44
  title: safety-policy-01ff452b1d44
  resource: ../sources/safety-policy-01ff452b1d44.md
sopkb:
  rule:
    id: rule-ki-safety-policy-01ff452b1d44-000015-requires
    type: SOP Decision Rule
    title: Contraindication Screening
    knowledge_item_id: ki-safety-policy-01ff452b1d44-000015
    source_id: safety-policy-01ff452b1d44
    section_id: section-safety-policy-01ff452b1d44-002
    review_status: deferred
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_contraindication_screening
      action: requires
      label: Patients with confirmed contraindications must not start GLP-1 therapy
        without specialist review.
    evidence_id: evidence-ki-safety-policy-01ff452b1d44-000015
    relation_id: kr-ki-safety-policy-01ff452b1d44-000015
    okf_path: rules/rule-ki-safety-policy-01ff452b1d44-000015-requires.md
  knowledge_piece: ../knowledge/ki-safety-policy-01ff452b1d44-000015.md
  knowledge_relation: ../relations/kr-ki-safety-policy-01ff452b1d44-000015.md
  evidence: ../evidence/evidence-ki-safety-policy-01ff452b1d44-000015.md
---
# Contraindication Screening

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_contraindication_screening`
- Review status: `deferred`

## Connected Knowledge

- Knowledge piece: [ki-safety-policy-01ff452b1d44-000015](../knowledge/ki-safety-policy-01ff452b1d44-000015.md)
- Knowledge relation: [kr-ki-safety-policy-01ff452b1d44-000015](../relations/kr-ki-safety-policy-01ff452b1d44-000015.md)
- Evidence: [evidence-ki-safety-policy-01ff452b1d44-000015](../evidence/evidence-ki-safety-policy-01ff452b1d44-000015.md)
