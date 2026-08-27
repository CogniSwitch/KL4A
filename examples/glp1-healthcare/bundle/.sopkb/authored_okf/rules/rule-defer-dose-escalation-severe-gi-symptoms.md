---
type: SOP Decision Rule
title: Defer dose escalation when severe gastrointestinal symptoms are reported
sources:
- id: follow-up-monitoring-procedure-38ef67176baf
  title: follow up monitoring procedure
  resource: follow-up-monitoring-procedure-38ef67176baf
sopkb:
  rule:
    id: rule-defer-dose-escalation-severe-gi-symptoms
    obligation:
      fact: dose_escalation_deferred
      action: defer
      label: Dose escalation deferred
    condition:
      fact: severe_gastrointestinal_symptoms_reported
      operator: is_true
      label: Severe gastrointestinal symptoms reported
generated:
  actor: sopkb/azure-llm
  date: '2026-08-10'
---

# Defer dose escalation when severe gastrointestinal symptoms are reported

Clinicians should defer dose escalation when severe gastrointestinal symptoms are reported.
