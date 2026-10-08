---
type: SOP Decision Rule
title: Manual Override Authorization
description: Hand-authored rule permitting a documented manual override.
resource: authored/overrides/manual-override.md
tags: []
status: stable
sopkb:
  rule:
    id: rule-manual-override
    title: Manual Override Authorization
    condition: null
    obligation:
      fact: manual-override
      action: obtain-supervisor-signoff
      label: Obtain supervisor sign-off before a manual override
---

A supervisor must co-sign any manual override of the standard SOP before it takes effect.
