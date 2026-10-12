---
title: Evaluation Demos
description: Repeatable, real-input evaluations for KL4A 0.0.2-alpha.
---

# Evaluation Demos

These are controlled evaluations, not product benchmarks. Each uses a public,
real-world input at a pinned version, starts with the released binaries, and
asks an agent to work only from the resulting bundle.

- [SOPKB: incident-response playbook](sopkb-cisa-incident-response.md)
- [CodeKB: FastAPI application](codekb-fastapi-template.md)

Run each in a new working directory. Record the KL4A asset name, operating
system, input checksum, command output, and any failure before changing the
input or retrying a command. That record is the evaluation result.
