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
  date: <TS>
---
# SOP KB Agent Guide

Use this bundle as a read-only task context unless a human review workflow explicitly enables writes.

## Consumption Flow

1. Read [Agent Task Contexts](../tasks/index.md) and choose the closest task.
2. Retrieve `agent.context` through CLI or MCP for usable knowledge, evidence, reports, concepts, and Knowledge Relations.
3. Follow links from knowledge pieces to evidence before making a claim.
4. Treat Knowledge Relations as RDF-compatible assertions connected to the supporting knowledge piece.
5. Check freshness and conflict reports before operational use.

## Decision Rules

- A knowledge item may carry a structured decision rule (condition/obligation/otherwise) in its `metadata.decision_rules`.
- `knowledge.search` flags this via a non-empty `rule_ids` field; `knowledge.get` and `agent.context` return the full rule.
- Wherever a matched item has a decision rule, fetch and apply that rule's condition/obligation logic — do not answer from the prose evidence alone when a structured rule exists.

## Safety Rules

- Do not use rejected knowledge unless explicitly asked to audit rejected material.
- Do not infer approval from generated knowledge. Human review is represented by `verified` metadata.
- Preserve unknown OKF frontmatter fields when updating documents.
