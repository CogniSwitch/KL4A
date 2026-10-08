---
type: SOP Decision Rule
title: Controls
description: Staff must confirm badge access before entering the restricted area.
resource: ../knowledge/ki-policy-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-policy
  title: policy
  resource: ../sources/policy.md
sopkb:
  rule:
    id: rule-ki-policy-v1-000001-requires
    type: SOP Decision Rule
    title: Controls
    knowledge_item_id: ki-policy-v1-000001
    source_id: policy
    section_id: section-policy-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_controls
      action: requires
      label: Staff must confirm badge access before entering the restricted area.
    evidence_id: evidence-ki-policy-v1-000001
    relation_id: kr-ki-policy-v1-000001
    okf_path: rules/rule-ki-policy-v1-000001-requires.md
  knowledge_piece: ../knowledge/ki-policy-v1-000001.md
  knowledge_relation: ../relations/kr-ki-policy-v1-000001.md
  evidence: ../evidence/evidence-ki-policy-v1-000001.md
---
# Controls

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_controls`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-policy-v1-000001](../knowledge/ki-policy-v1-000001.md)
- Knowledge relation: [kr-ki-policy-v1-000001](../relations/kr-ki-policy-v1-000001.md)
- Evidence: [evidence-ki-policy-v1-000001](../evidence/evidence-ki-policy-v1-000001.md)
