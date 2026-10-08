---
type: SOP Agent Task Context
title: Follow-Up Monitoring
description: Use SOP requirements to identify follow-up observations and review triggers.
resource: tasks/follow-up-monitoring.md
tags:
- agent-task
- follow-up-monitoring
status: stable
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sopkb:
  task_id: follow-up-monitoring
  query_terms:
  - follow-up
  - follow up
  - dose
  - tolerance
  - adverse
  - adherence
  agent_cli: sopkb agent context <bundle_dir> --task follow-up-monitoring
---
# Follow-Up Monitoring

Use SOP requirements to identify follow-up observations and review triggers.

## Agent Use

- Retrieve context with `sopkb agent context <bundle_dir> --task follow-up-monitoring`.
- Use returned Knowledge Relations for graph traversal.
- Resolve evidence before applying any rule.
