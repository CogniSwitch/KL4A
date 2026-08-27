---
type: SOP Section
title: Lone Hash Adopts This Line As Heading Text
description: Normalized section from weird headings.
resource: ../../sources/weird-headings.md
tags:
- section
- normalized
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-weird-headings
  title: weird headings
  resource: ../../sources/weird-headings.md
sopkb:
  section_id: section-weird-headings-001
  source_id: weird-headings
  source_version_id: weird-headings:v1
  ordinal: 1
  normalized_path: sources/normalized/weird-headings__v1.md
---
# Lone Hash Adopts This Line As Heading Text

Source: [weird headings](../../sources/weird-headings.md)

## Knowledge Pieces

- No knowledge pieces mined from this section.

## Source Excerpt

#NoSpaceHeading

This line is body text, not a section boundary — "#NoSpaceHeading" has no space after the hash so it never matches the heading regex.

####### Seven Hashes

This paragraph is not heading-adjacent either: `#{1,6}` cannot match 7 leading hashes at all, so the line above is ordinary text.

#
Lone Hash Adopts This Line As Heading Text

## Purpose ##

Staff must confirm the escalation procedure described below.

```
# not a real heading, inside a fence
Clinicians should route uncertain cases for review.
```

## Procedure

Staff should record contraindications and route cases per policy.
