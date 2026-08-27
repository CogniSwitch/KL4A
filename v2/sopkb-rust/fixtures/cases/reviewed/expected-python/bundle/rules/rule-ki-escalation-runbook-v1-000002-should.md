---
type: SOP Decision Rule
title: Secondary Contact
description: Clinicians should route unresolved alerts to the secondary on-call physician.
resource: ../knowledge/ki-escalation-runbook-v1-000002.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-escalation-runbook
  title: escalation runbook
  resource: ../sources/escalation-runbook.md
sopkb:
  rule:
    id: rule-ki-escalation-runbook-v1-000002-should
    type: SOP Decision Rule
    title: Secondary Contact
    knowledge_item_id: ki-escalation-runbook-v1-000002
    source_id: escalation-runbook
    section_id: section-escalation-runbook-003
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_secondary_contact
      action: should
      label: Clinicians should route unresolved alerts to the secondary on-call physician.
    evidence_id: evidence-ki-escalation-runbook-v1-000002
    relation_id: kr-ki-escalation-runbook-v1-000002
    okf_path: rules/rule-ki-escalation-runbook-v1-000002-should.md
  knowledge_piece: ../knowledge/ki-escalation-runbook-v1-000002.md
  knowledge_relation: ../relations/kr-ki-escalation-runbook-v1-000002.md
  evidence: ../evidence/evidence-ki-escalation-runbook-v1-000002.md
---
# Secondary Contact

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_secondary_contact`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-escalation-runbook-v1-000002](../knowledge/ki-escalation-runbook-v1-000002.md)
- Knowledge relation: [kr-ki-escalation-runbook-v1-000002](../relations/kr-ki-escalation-runbook-v1-000002.md)
- Evidence: [evidence-ki-escalation-runbook-v1-000002](../evidence/evidence-ki-escalation-runbook-v1-000002.md)
