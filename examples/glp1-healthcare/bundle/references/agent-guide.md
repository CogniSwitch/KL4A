---
type: SOP Agent Guide
title: SOP KB Agent Guide
description: How agents should consume this OKF bundle.
resource: references/agent-guide.md
tags:
- agent-guide
- reference
status: stable
generated:
  actor: sopkb/0.1.0
  date: '2026-08-10'
---
# SOP KB Agent Guide

Use this bundle as a read-only task context unless a human review workflow explicitly enables writes.

## Consumption Flow

1. Read [Agent Task Contexts](../tasks/index.md) and choose the closest task.
2. Retrieve `agent.context` through CLI or MCP for usable knowledge, evidence, reports, concepts, and Knowledge Relations.
3. Follow links from knowledge pieces to evidence before making a claim.
4. Treat Knowledge Relations as RDF-compatible assertions connected to the supporting knowledge piece.
5. Check freshness and conflict reports before operational use.

## Safety Rules

- Do not use rejected knowledge unless explicitly asked to audit rejected material.
- Do not infer approval from generated knowledge. Human review is represented by `verified` metadata.
- Preserve unknown OKF frontmatter fields when updating documents.
