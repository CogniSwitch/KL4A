---
type: SOP Decision Rule
title: Intake Requirements
description: Clinicians must document current medications, diabetes history, weight
  history, and relevant comorbidities.
resource: ../knowledge/ki-primary-care-glp1-sop-c6cb3b4ec11d-000006.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: '2026-08-10'
sources:
- id: src-primary-care-glp1-sop-c6cb3b4ec11d
  title: primary-care-glp1-sop-c6cb3b4ec11d
  resource: ../sources/primary-care-glp1-sop-c6cb3b4ec11d.md
sopkb:
  rule:
    id: rule-ki-primary-care-glp1-sop-c6cb3b4ec11d-000006-requires
    type: SOP Decision Rule
    title: Intake Requirements
    knowledge_item_id: ki-primary-care-glp1-sop-c6cb3b4ec11d-000006
    source_id: primary-care-glp1-sop-c6cb3b4ec11d
    section_id: section-primary-care-glp1-sop-c6cb3b4ec11d-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_intake_requirements
      action: requires
      label: Clinicians must document current medications, diabetes history, weight
        history, and relevant comorbidities.
    evidence_id: evidence-ki-primary-care-glp1-sop-c6cb3b4ec11d-000006
    relation_id: kr-ki-primary-care-glp1-sop-c6cb3b4ec11d-000006
    okf_path: rules/rule-ki-primary-care-glp1-sop-c6cb3b4ec11d-000006-requires.md
  knowledge_piece: ../knowledge/ki-primary-care-glp1-sop-c6cb3b4ec11d-000006.md
  knowledge_relation: ../relations/kr-ki-primary-care-glp1-sop-c6cb3b4ec11d-000006.md
  evidence: ../evidence/evidence-ki-primary-care-glp1-sop-c6cb3b4ec11d-000006.md
---
# Intake Requirements

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_intake_requirements`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-primary-care-glp1-sop-c6cb3b4ec11d-000006](../knowledge/ki-primary-care-glp1-sop-c6cb3b4ec11d-000006.md)
- Knowledge relation: [kr-ki-primary-care-glp1-sop-c6cb3b4ec11d-000006](../relations/kr-ki-primary-care-glp1-sop-c6cb3b4ec11d-000006.md)
- Evidence: [evidence-ki-primary-care-glp1-sop-c6cb3b4ec11d-000006](../evidence/evidence-ki-primary-care-glp1-sop-c6cb3b4ec11d-000006.md)
