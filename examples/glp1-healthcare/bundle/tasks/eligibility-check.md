---
type: SOP Agent Task Context
title: Eligibility Check
description: Use SOP requirements to decide what eligibility facts must be gathered
  or verified.
resource: tasks/eligibility-check.md
tags:
- agent-task
- eligibility-check
status: stable
generated:
  actor: sopkb/0.1.0
  date: '2026-08-10'
sopkb:
  task_id: eligibility-check
  query_terms:
  - eligibility
  - identity
  - contraindication
  - clinical review
  agent_cli: sopkb agent context <bundle_dir> --task eligibility-check
---
# Eligibility Check

Use SOP requirements to decide what eligibility facts must be gathered or verified.

## Agent Use

- Retrieve context with `sopkb agent context <bundle_dir> --task eligibility-check`.
- Use returned Knowledge Relations for graph traversal.
- Resolve evidence before applying any rule.
