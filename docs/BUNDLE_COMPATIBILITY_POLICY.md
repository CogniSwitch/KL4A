---
title: Bundle Compatibility Policy
---

# Bundle Compatibility Policy

## 1. Why This Is Separate From Software SemVer

`sopkb` (the software) and the SOP Knowledge Bundle / OKF-based bundle format
(the artifact) are versioned independently, on purpose.

The bundle is meant to be a **durable artifact**. Once someone builds a bundle:

- reviews it
- checks it into Git
- hands it to an agent
- exports it toward an enterprise import path

That bundle needs to keep working, keep being readable, and keep being
loadable by the tools that produced it, largely independent of how fast the
`sopkb` software itself is moving.

If bundle-format compatibility were tied to the software's SemVer number,
a routine `0.0.1 -> 0.1.0` software release (which, pre-1.0, carries no
strong compatibility guarantee at all per `docs/RELEASE_PROCESS.md`) could
silently invalidate every bundle a user has already built. That is the
outcome this policy exists to prevent:

!!! warning "The rule this policy enforces"
    A minor software version bump must never silently break an existing
    bundle's schema. Any bundle-breaking change must be a deliberate,
    documented, versioned event in the bundle format's own numbering — not
    a side effect of a software release.

## 2. Current Bundle Format Version

The canonical bundle shape is specified in `docs/OKF_BUNDLE_SPEC.md`. As of
this writing, that document states:

```text
Status: draft
Spec version: 0.2.0
Target OKF version: 0.2
```

That spec is explicitly marked `draft` and is expected to keep evolving prior
to a 1.0 bundle-format release, independent of the `sopkb` software version
(currently `0.0.1`). Manifests already carry their own `profile_version`
field (see `manifest.yaml`'s `profile: sop-knowledge-bundle` /
`profile_version` in `docs/OKF_BUNDLE_SPEC.md` Section 7), separate from
`okf_version` and from the software version recorded under
`generated.actor` (e.g. `sopkb/0.0.1`).

## 3. Versioning Scheme for the Bundle Format

The SOP Knowledge Bundle format has its own `MAJOR.MINOR` version,
independent of the `sopkb` software's SemVer:

```text
bundle format version: MAJOR.MINOR
```

There is no `PATCH` component: bundle-format changes are structural
(directory layout, frontmatter contract, required fields, id semantics), not
the kind of thing that has a meaningful patch-level fix distinct from a minor
revision. A change either alters the schema (bump `MINOR` or `MAJOR`) or it
doesn't (no version change, e.g. wording-only spec clarifications that don't
change what a valid bundle looks like).

| Bump | Meaning | Examples |
|------|---------|----------|
| **`MINOR`** | Backwards-compatible bundle-format change. Existing bundles at the previous minor version remain valid and readable by tooling that supports the new minor version. | Adding a new optional frontmatter field; adding a new optional document type; adding a new `reports/` file. |
| **`MAJOR`** | Breaking bundle-format change. A bundle built under the old major version is not guaranteed to validate or be consumable as-is under tooling that only supports the new major version without migration. | Renaming/removing a required frontmatter field; changing the canonical directory shape; changing id derivation rules; changing what `rdf_compatible: true` implies structurally. |

This bundle-format version is recorded today as the `Spec version` field in
`docs/OKF_BUNDLE_SPEC.md` and should also be reflected in each bundle's
`manifest.yaml` via its `profile_version` field, so a bundle is
self-describing about which schema revision it was written against. The
separate `okf_version` field tracks alignment with the external Open
Knowledge Format standard and is not the same axis as `profile_version`;
both may need to be read together when reasoning about compatibility.

## 4. Breaking-Change Policy

!!! danger "Breaking change — requires a `MAJOR` bump"
    A bundle-format change is a breaking change if it would cause any of the
    following for a bundle that validated successfully under the previous
    version:

    - A required canonical directory or file (e.g. `index.md`, `manifest.yaml`,
      a document under `knowledge/`, `evidence/`, `relations/`) is removed,
      relocated, or renamed.
    - A required `sopkb` frontmatter field for an existing document type is
      removed, renamed, or given a different meaning.
    - A previously valid document would fail bundle validation
      (`sopkb validate`) under the new spec without modification.
    - The semantics of an existing field change such that old and new bundles
      disagree on meaning for the same field name (e.g. a status enum value is
      redefined rather than extended).
    - id derivation for `source_id`, `knowledge_item_id`, `concept_id`, etc.
      changes such that previously generated ids no longer resolve consistently.

!!! success "Non-breaking change — `MINOR` bump, or no bump"
    A change is non-breaking if it only adds optional structure that old
    bundles simply don't have yet, or clarifies wording without changing what
    a conformant bundle must contain.

Any change proposed against `docs/OKF_BUNDLE_SPEC.md` should be classified
against this list as part of its review, and the `Spec version` header in
that document should be updated accordingly in the same change.

## 5. Migration Note Requirement

!!! warning "Migration note required for every `MAJOR` bump"
    Every breaking (`MAJOR`) bundle-format change MUST ship with a migration
    note. Concretely:

    - A short Markdown migration note describing:
        - what changed
        - why
        - which bundles are affected (by prior bundle-format version)
        - the concrete steps (manual or via a `sopkb migrate`-style tool, once one exists) to bring an existing bundle up to the new format version
    - The migration note should live alongside the bundle-format history (for
      example under a `docs/bundle-migrations/` directory, added when the first
      breaking change actually happens) and should be linked from the updated
      `docs/OKF_BUNDLE_SPEC.md`.
    - The corresponding software release notes (`docs/releases/<version>.md`,
      see `docs/RELEASE_PROCESS.md`) should call out that a bundle-format
      breaking change shipped in that release and link to the migration note.

    No `MAJOR` bundle-format bump should land without this note. A `MINOR`
    bump does not require a migration note, since existing bundles remain
    valid.

## 6. Relationship to Software Releases

- A software release (`sopkb` version bump per `docs/RELEASE_PROCESS.md`)
  and a bundle-format version bump are independent events. A software patch
  or minor release can ship with no bundle-format change at all.
- When a software release does change the bundle-format version, that must
  be stated explicitly in that release's notes under `docs/releases/`,
  including the old and new bundle-format version and whether it was a
  `MINOR` (non-breaking) or `MAJOR` (breaking, migration note required)
  bundle-format bump.
- Software pre-1.0 status (see `docs/RELEASE_PROCESS.md`) does not relax this
  policy. Even while `sopkb` is `0.x`, bundle-format breaking changes still
  require a bundle-format `MAJOR` bump and a migration note — the two version
  numbers are deliberately decoupled in both directions.

## 7. Pre-1.0 Bundle Format Expectations

The bundle format is currently pre-1.0 (`0.2.0` per `docs/OKF_BUNDLE_SPEC.md`,
status `draft`). Users should expect the schema may still shift as the format
matures toward a `1.0` bundle-format release, at which point the compatibility
guarantees in Section 4 above become the project's durable commitment. Until
then, breaking changes are still governed by this policy (classified,
versioned, and given a migration note) — "pre-1.0" describes how much change
to expect, not a waiver of the process for making that change.
