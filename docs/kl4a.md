---
title: kl4a — Unified CLI and Library
description: >-
  kl4a is one PyPI package containing both tools - sopkb and codekb - plus a
  dispatcher CLI and import namespace in front of them.
---

# kl4a — one package, one install, two tools

**KL4A** is the product. [`sopkb`](sopkb/quickstart.md) and [`codekb`](codekb/index.md)
are the two options it offers - each its own importable package and its own
console script, each runnable on its own. Both, plus the shared OKF core they
build on, ship inside **one PyPI project:
`kl4a`.** There is nothing else to install.

`kl4a` also gives you an optional dispatcher: one command and one import root
that picks which of the two tools to run, so you don't have to remember
two separate CLI names or two separate import roots. The dispatcher adds
no behavior of its own - `sopkb` and `codekb` still work exactly the
same when run directly.

## What it is, and isn't

- **It is** one PyPI project shipping the shared OKF core - bundle model,
  storage, identifiers, OKF documents, LLM provider settings - together with
  the `sopkb` and `codekb` packages that build on it, plus a thin
  dispatcher CLI (`kl4a --use TOOL ...`) and an import namespace
  (`kl4a.sopkb`, `kl4a.codekb`) over them.
- **It isn't** a fourth tool with its own subcommands, flags, or behavior.
  `sopkb` and `codekb` keep working exactly as before, with their own
  console scripts (`sopkb`, `codekb`) and their own imports -
  the dispatcher and aliasing layer change nothing about them.
- A single `pip install kl4a` gets you both tools and everything they
  depend on - nothing is gated behind an extra.

## Install

```bash
pip install kl4a
```

That's the whole install. `sopkb` and `codekb` both come with it, as
do their dependencies (`pdfplumber`/`python-docx` for `sopkb`'s document
parsing included) - there's no extra to remember and no partial install.

From a checkout instead, it's one editable install of the one package:

```bash
pip install -e tools/kl4a
```

## CLI usage

Two ways to run either tool, once `kl4a` is installed - both reach
the exact same code:

```bash
sopkb init demo-bundle                 # the tool's own console script
kl4a --use sopkb init demo-bundle      # -> exactly `sopkb init demo-bundle`
```

`kl4a --use TOOL ...` forwards everything after `--use TOOL` to that tool's
own `main()` completely unchanged — same subcommands, same flags, same
`--help`, same exit codes:

```bash
kl4a --use sopkb init demo-bundle      # -> exactly `sopkb init demo-bundle`
kl4a --use codekb build ./my-repo      # -> exactly `codekb build ./my-repo`
```

`kl4a --use sopkb --help` prints `sopkb`'s own help verbatim — the usage line
reads `usage: sopkb ...`, not a summary of it written by `kl4a`:

```
$ kl4a --use sopkb --help
usage: sopkb [-h]
             {init,scan,normalize,validate,mine,export,bundle,sources,...} ...

SOP Knowledge Bundle workbench
...
```

The same holds for `kl4a --use codekb --help` (`usage: codekb ...`).

Running `kl4a` with no `--use` prints `kl4a`'s own help and exits `1`, since
there's nothing to dispatch to:

```
$ kl4a

error: --use is required (sopkb or codekb)
usage: kl4a [--use {sopkb,codekb}] [-h]
...
```

`kl4a --help` (or `-h`) with no `--use` prints the same help but exits `0`,
since `--help` was explicitly asked for rather than defaulted into.

Each tool is imported lazily, only on first use of `--use <that tool>` - a
minor startup-cost saving, not an install-avoidance one: `pip install kl4a`
already gave you both.

## Library usage

```python
import kl4a.sopkb
import kl4a.codekb
```

Each of these is the real installed package reachable under the `kl4a`
namespace — not a copy, not a re-export built name by name. Once both are
imported, `kl4a.sopkb is sopkb` (the same object), all the way down:
`kl4a.sopkb.cli.main is sopkb.cli.main` is also `True`. Anything added to
`sopkb` later — a new function, a new submodule — is visible under
`kl4a.sopkb` too, with no change needed here.

## Why use `kl4a --use TOOL` instead of `sopkb`/`codekb` directly?

You don't have to. There is nothing `kl4a --use TOOL` can do that the tool's
own CLI or import can't already do alone — it adds no behavior, no flags, no
output format of its own. The only thing it buys you is not having to
remember which of two command names or two import roots you need for a
given job. If you already know you only ever use one of the two, run and
import that one directly and skip the dispatcher entirely - either way, it's
the same one `pip install kl4a`.

## Experimental: a Rust implementation exists too

!!! info "Not the published package — a separate, experimental effort"
    Everything above describes the officially released `kl4a` you get from
    `pip install kl4a`. This section is about something else entirely: an
    experimental, from-scratch Rust reimplementation living at `kl4a-rs/` in
    this repository, not yet published anywhere.

`kl4a-rs/` contains a Rust port of `codekb` (with a shared `kl4a-core` crate),
plus a separately-developed, more mature Rust port of
`sopkb` (`kl4a-rs/sopkb-rust/`), and a `kl4a` dispatcher binary tying both
together with the **same `--use TOOL` command style** described above —
but built the Rust way, not a copy of this page's Python mechanics: true
in-process calls into each tool's own CLI entry point (`codekb::cli::main`,
`sopkb_cli::execute`), the same pattern `codekb.exe` already uses on its own,
rather than Python's lazy-import
forwarding. One documented exception: `kl4a --use sopkb serve`/`mcp serve`
have no in-process entry point in `sopkb-cli` to call into, so those two
specifically spawn the sibling `sopkb-server`/`sopkb-mcp` binaries as
subprocesses instead.

```bash
kl4a --use sopkb init demo-bundle      # same style as the Python dispatcher,
kl4a --use codekb build ./my-repo      # different mechanism underneath
```

Parity status, what's verified, and what's still open are tracked in
[Rust Port Parity](codekb/rust-port-parity.md) (codekb) and the
[Roadmap](../ROADMAP.md) (the web UI this groundwork is for). None of this
is a commitment that the Rust port will replace or ship alongside the
Python package — it's groundwork being evaluated, not a release.

## See also

- [`sopkb` Quickstart](sopkb/quickstart.md) — what the documents option does
  once dispatched to.
- [`codekb` Overview](codekb/index.md) — what the source-code option does
  once dispatched to.
