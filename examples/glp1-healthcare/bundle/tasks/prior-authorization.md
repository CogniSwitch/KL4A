---
type: SOP Agent Task Context
title: Prior Authorization Packet
description: Use SOP requirements to assemble payer, diagnosis, and documentation
  evidence.
resource: tasks/prior-authorization.md
tags:
- agent-task
- prior-authorization
status: stable
generated:
  actor: sopkb/0.1.0
  date: '2026-08-10'
sopkb:
  task_id: prior-authorization
  query_terms:
  - prior authorization
  - payer
  - diagnosis
  - evidence
  - packet
  agent_cli: sopkb agent context <bundle_dir> --task prior-authorization
---
# Prior Authorization Packet

Use SOP requirements to assemble payer, diagnosis, and documentation evidence.

## Agent Use

- Retrieve context with `sopkb agent context <bundle_dir> --task prior-authorization`.
- Use returned Knowledge Relations for graph traversal.
- Resolve evidence before applying any rule.
