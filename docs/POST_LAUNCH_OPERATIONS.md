---
title: Post-Launch Operations
description: >-
  How the project is run after launch: triage, support expectations and the cadence
  for keeping bundles and docs current.
---

# Post-Launch Operations

Internal maintainer playbook for running `sopkb` day-to-day after the open-source launch. This is not user-facing documentation — it's the checklist maintainers follow to keep the project healthy.

## Issue / PR Triage Cadence

Pick a cadence and stick to it.

!!! warning "Recommended cadence: weekly (biweekly is the absolute floor)"

    At least while the project is small, weekly is recommended — do not go longer than biweekly.

Stale, unattended issues and PRs are the fastest way for a new OSS project to look abandoned. A project with a two-day-old commit and a three-month-old unanswered issue reads as dead to a new contributor, regardless of how active development actually is. Consistency matters more than thoroughness here: a quick weekly pass beats a thorough pass that happens every couple of months.

What "done" looks like for a triage pass:
- Every open issue and PR touched since the last pass has been looked at — none left completely untouched.
- Each item is either labeled, assigned, or explicitly deferred (e.g. commented "not planned for now, revisiting after vX.Y" and labeled accordingly).
- Duplicate or stale items are closed with a short note rather than left to rot.
- Anything that needs a maintainer decision is flagged as such so it doesn't silently stall.

## Security Reports

!!! warning "SLA: acknowledge within 3 business days"

    Acknowledge every security report within 3 business days, even if the actual fix takes longer. The acknowledgment is a status update, not a resolution — "we've seen this, we're investigating, we'll follow up by [date]" is sufficient to stop the clock on the SLA.

This cadence pairs with (but does not replace):
- The disclosure process described in `SECURITY.md` (owned separately — reference it, don't duplicate it here).
- GitHub Security Advisories for coordinated disclosure and, where applicable, CVE assignment.

If a fix is going to take longer than the initial acknowledgment implied, send a follow-up update rather than letting the reporter wonder whether the report went into a void.

## Roadmap Cadence

Revisit and republish the public roadmap every **4-6 weeks**. See `ROADMAP.md` for the current roadmap content (owned by another workstream — this document only sets the cadence for revisiting it, not its content).

The point of the cadence is to prove the roadmap is a living document, not a one-time launch announcement. A roadmap that hasn't moved in three months signals a stalled project even if work is happening elsewhere (commits, releases, triage). Each revisit should at minimum:
- Move completed items off (or mark them done).
- Reflect any priority changes based on issue/PR volume from triage.
- Get republished (announcement, pinned issue update, or discussion post) so the update is visible, not just committed to the repo.

## Summary Cadences

| Activity | Cadence | Notes |
|---|---|---|
| Issue/PR triage | Weekly (biweekly floor) | See "done" checklist above |
| Security report acknowledgment | Within 3 business days | Pairs with `SECURITY.md` + GitHub Security Advisories |
| Roadmap revisit/republish | Every 4-6 weeks | Roadmap content lives in `ROADMAP.md` |
