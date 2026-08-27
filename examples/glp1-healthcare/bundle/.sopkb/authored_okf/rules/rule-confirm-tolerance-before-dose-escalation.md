---
type: SOP Decision Rule
title: Confirm patient tolerance before dose escalation
sources:
- id: follow-up-monitoring-procedure-38ef67176baf
  title: follow up monitoring procedure
  resource: follow-up-monitoring-procedure-38ef67176baf
sopkb:
  rule:
    id: rule-confirm-tolerance-before-dose-escalation
    obligation:
      fact: patient_tolerance_confirmed
      action: confirm
      label: Patient tolerance confirmed
    condition:
      fact: dose_escalation_planned
      operator: is_true
      label: Dose escalation planned
generated:
  actor: sopkb/azure-llm
  date: '2026-08-10'
---

# Confirm patient tolerance before dose escalation

Clinicians must confirm patient tolerance before dose escalation.
