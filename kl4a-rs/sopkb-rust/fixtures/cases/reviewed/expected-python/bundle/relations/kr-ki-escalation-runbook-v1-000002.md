---
type: SOP Knowledge Relation
title: kr-ki-escalation-runbook-v1-000002
description: Secondary Contact should Clinicians should route unresolved alerts to
  the secondary on-call.
resource: ../knowledge/ki-escalation-runbook-v1-000002.md
tags:
- relation
- rdf-compatible
- should
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-escalation-runbook
  title: escalation runbook
  resource: ../sources/escalation-runbook.md
sopkb:
  relation:
    id: kr-ki-escalation-runbook-v1-000002
    type: Knowledge Relation
    subject:
      id: concept-secondary-contact
      label: Secondary Contact
      text: Secondary Contact
      okf_path: concepts/concept-secondary-contact.md
    predicate:
      id: predicate-should
      text: should
    object:
      id: object-clinicians-should-route-unresolved-alerts-to-the-secondary-on-ca
      text: Clinicians should route unresolved alerts to the secondary on-call physician.
      label: Clinicians should route unresolved alerts to the secondary on-call
    knowledge_piece_id: ki-escalation-runbook-v1-000002
    evidence_id: evidence-ki-escalation-runbook-v1-000002
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-escalation-runbook-v1-000002

## Assertion

- Subject: [Secondary Contact](../concepts/concept-secondary-contact.md)
- Predicate: `should`
- Object: Clinicians should route unresolved alerts to the secondary on-call physician.

## Connected Knowledge

- Knowledge piece: [ki-escalation-runbook-v1-000002](../knowledge/ki-escalation-runbook-v1-000002.md)
- Evidence: [evidence-ki-escalation-runbook-v1-000002](../evidence/evidence-ki-escalation-runbook-v1-000002.md)
- Decision rule: [Secondary Contact](../rules/rule-ki-escalation-runbook-v1-000002-should.md)
