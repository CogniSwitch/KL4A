---
title: Code Knowledge and Review
description: >-
  Claim tiers, the review queue and the non-destructive review overlay that decides
  which code claims an agent is allowed to retrieve.
---

# Knowledge & Review

A **claim** is a statement about the code, bound to the symbols it concerns and to
the evidence it was derived from. Claims are the layer agents consume; everything
below them is structure.

## Tiers

| Tier | Content | Review |
| --- | --- | --- |
| 3 | Behaviour and documentation claims | Not required by default |
| 4 | Architecture, workflow and safety claims | Flagged for review |

Tier 4 is the material an agent would act on most consequentially, which is why it
is the tier that carries a review gate.

## Anchoring

Every claim records how firmly it is attached to real source:

| Anchor status | Means |
| --- | --- |
| `exact` | The quoted code was located in the symbol's own lines |
| `llm_claimed` | The model quoted something that could not be located |
| `unknown` | No anchor was recorded |

An `llm_claimed` claim is not discarded — it is demoted to Tier 4 and surfaced as
**unanchored** in the review queue, because a plausible claim whose quote does not
exist is exactly what a human should look at.

## Lifecycle

`active` claims describe the current bundle. `superseded` and `retired` claims
describe an earlier version of the code, so every review filter but one leaves
them out — there is nothing useful to decide about them. They are not lost: the
`retired` filter lists them, and the Overview's **retired claims** tile links
straight to it.

## Reviewing

The queue switcher offers six views, five of them over the active claims:

| Filter | Contains |
| --- | --- |
| `all` | Every active claim |
| `review_required` | Tier 4, or explicitly flagged |
| `proposed` | Not yet decided |
| `anchored` | Quote located, still proposed |
| `unanchored` | Quote could not be located |
| `retired` | `superseded` and `retired` claims — the only view of the non-active pool |

Group by module or by claim kind to judge like with like — clearing every
docstring claim in a module in one pass beats reading an undifferentiated list.

### Decisions

**Approve**, **defer**, **reject** or **comment**. A rationale is required; approval
is never inferred.

Decisions persist to `.codekb/code_reviews.json` as a **non-destructive overlay** —
the mined claims are never rewritten, so a rebuild does not lose your decisions.

A batch applies one decision, with one rationale, to everything selected, while
still writing one event per claim tagged with the batch size. The history stays
per-claim and auditable.

## What Reaches An Agent

`code.context` filters claims before returning them. A claim is excluded when:

- it has been **rejected**, or
- its lifecycle status is not **active**, or
- it is **Tier 4 or above, flagged as requiring review, and not approved**.

Approval is what opens the tier 4 gate. Deferring or commenting leaves it closed:
only an explicit approval widens retrieval.

!!! note "Decisions are an overlay"
    Approving a claim does not rewrite it. The decision is recorded in
    `.codekb/code_reviews.json` and applied when the claim is read, which is what
    lets a re-mine preserve your review. Retrieval applies the same overlay the
    code context does, so both agree on a claim's status.

This filter is the reason unreviewed enrichment is wasted spend: hybrid mining
that is paid for and never reviewed produces claims that retrieval will not use.

## Commands

```bash
# the queue, as data
kl4a --use codekb mine ./b            # (re)generate claims
kl4a --use codekb validate ./b        # includes review-required counts
```

There is no `codekb review` command in this release. The overlay file is plain
JSON if you need to inspect recorded decisions:

```bash
cat ./b/.codekb/code_reviews.json
```
