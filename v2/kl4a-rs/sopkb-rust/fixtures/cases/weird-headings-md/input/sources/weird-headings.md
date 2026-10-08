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
