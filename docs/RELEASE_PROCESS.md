---
title: Release Process
description: >-
  How a KL4A release is cut, versioned and published, and what is checked before it
  goes out.
---

# Release Process

This document describes how `kl4a` (the package under `tools/kl4a/`) is
versioned, tagged, and released. `kl4a` is the only PyPI project this repo
publishes — `sopkb` and `codekb` ship inside the same distribution
rather than as separate projects, so there is exactly one version, one tag,
one build, and one publish step.

## 1. Versioning

`kl4a` follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html),
starting at `0.1.0`.

While the package is in the `0.x` series, the following should all be
considered pre-1.0 and may still change:

- the public API
- CLI surface
- web app
- MCP tool surface
- and — importantly — the on-disk SOP Knowledge Bundle format

A `0.x.y` bump does not carry the same stability guarantee that a post-1.0
release will.

The bundle format itself (the OKF-based directory layout documented in
`docs/OKF_BUNDLE_SPEC.md`) is versioned separately from the software. See
`docs/BUNDLE_COMPATIBILITY_POLICY.md` for how bundle-format compatibility is
tracked and communicated independently of the software's SemVer number.

In short:

`MAJOR`
:   Incompatible API/CLI changes once the project is post-1.0.

`MINOR`
:   Backwards-compatible functionality additions.

`PATCH`
:   Backwards-compatible bug fixes.

Pre-1.0 (`0.x.y`)
:   Any of the above may include breaking changes; treat `0.x` bumps as
    informative, not as a stability contract.

## 2. Tag Convention

Releases are triggered by pushing an annotated Git tag of the form:

```text
vX.Y.Z
```

for example `v0.1.0`, `v0.2.0`, `v1.0.0`. The tag version must match the
`version` field in `tools/kl4a/pyproject.toml` at the commit being tagged;
the release pipeline enforces this (see the `verify_version` job in
`.github/workflows/release.yml`).

!!! warning "No separate publish step"
    Pushing a `v*` tag is the single trigger for the release pipeline —
    there is no separate manual "publish" step.

## 3. What the Tag Triggers

Pushing a `vX.Y.Z` tag runs the release pipeline (implemented as a GitHub
Actions workflow), which:

1. **Builds** distribution artifacts (sdist and wheel) for `kl4a`, from
   `tools/kl4a/` — `sopkb` and `codekb` are packaged inside that
   same build, not built separately.
2. **Publishes** that one artifact set to PyPI using trusted publishing (no
   long-lived API tokens stored in the repository).
3. **Generates GitHub Release notes** for the tag from `CHANGELOG.md` at the
   repository root, and attaches the build artifacts to the GitHub Release.

!!! note "CHANGELOG.md must already be ready"
    The pipeline only *reads* the entry for the released version out of
    `CHANGELOG.md` — it does not author or edit that file, and this document
    does not either. `CHANGELOG.md` must be updated separately, ahead of the
    tag being cut.

## 4. Release Checklist

Before tagging a release:

- [ ] Confirm `tools/kl4a/pyproject.toml` `version` has been bumped to the
      release version (the release pipeline fails the tag push otherwise).
- [ ] Confirm `CHANGELOG.md` has a heading for that version (not just
      "unreleased") summarizing what changed.
- [ ] Add a dedicated release notes file under `docs/releases/` for the
      version being released, e.g. `docs/releases/v0.1.0.md`. This file is
      release-notes prose aimed at users (what's new, what's out of scope,
      known caveats, where to discuss) rather than a changelog diff — see
      `docs/releases/v0.1.0.md` for the first example. Every tagged release
      should have a corresponding file here.
- [ ] Ensure the working tree on the target branch is clean and CI is green.

To cut the release:

```text
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

Pushing the tag hands off to the release pipeline described in Section 3.

## 5. Post-Release

- [ ] Verify the package appears on PyPI at the expected version.
- [ ] Verify the GitHub Release was created with notes derived from
      `CHANGELOG.md`.
- [ ] Confirm `docs/releases/<version>.md` is present and linked/discoverable
      for users landing on the release.
- [ ] Where relevant (see the per-version release notes file), announce the
      release to the communities it's relevant to.

## 6. Scope of This Document

This document covers *when and how a release is cut and published*. It does
not cover:

- The content policy for `CHANGELOG.md` (maintained on its own).
- Bundle-format compatibility and migration policy — see
  `docs/BUNDLE_COMPATIBILITY_POLICY.md`.
- CI/CD workflow implementation details — see the workflow definitions under
  `.github/workflows/`.
