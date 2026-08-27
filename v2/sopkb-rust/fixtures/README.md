# fixtures

The differential test harness's fixture corpus (PORT_PLAN.md §6.0, V1).

```
golden/       committed, never regenerated: a pristine copy of examples/glp1-healthcare/bundle
              plus 4-6 synthetic bundles
harness/harness.py   the driver (list-cases / record / run / run-all)
cases/<name>/
  input/               source tree + starting bundle state
  script               ordered command sequence to run
  NOTES.md             (only where a case needs one — see bom-md, case-only-names)
  expected-python/     produced by the Python CLI — NOT YET GENERATED, see below
```

## Status: Phase 0 gate passed, re-baselined against current oss-launch (2026-08-21)

All 21 required cases from PORT_PLAN.md §6.0's table are authored: `reference`, `ascii-md`,
`unicode-md`, `bom-md`, `crlf-md`, `no-heading-md`, `empty-md`, `preamble-md`, `weird-headings-md`,
`colliding-stems`, `case-only-names`, `unsupported-types`, `legacy-layout`, `missing-empty-dirs`,
`reviewed`, `authored-okf`, and the five `malformed-*` sub-cases (`malformed-null-manifest-sources`,
`malformed-null-confidence`, `malformed-items-object`, `malformed-inventory-array`,
`malformed-warnings-strings`). Each has `input/` and `script`; `harness/harness.py` implements
`list-cases`/`record`/`run`/`run-all` per §6.0's V1 algorithm (byte-exact diff, normalizing only
timestamps → `<TS>` and host-absolute paths → `<ABS>`, nothing else).

`expected-python/` has been generated and verified for all 20 applicable cases (`case-only-names`
correctly `SKIP`s on a case-insensitive filesystem — see its `NOTES.md`). Phase 0's own "done when"
criterion — *the harness runs Python-against-Python and reports zero diffs for every case* — is
met: `python v2/sopkb-rust/fixtures/harness/harness.py run-all --engine python` reports `PASS` for
all 20 and exits 0, confirmed on two consecutive fresh runs.

**Re-baselined 2026-08-21 against current oss-launch.** Per `docs/port/CATCHUP_PLAN.md` decision
D1, `expected-python/` was regenerated for all 20 applicable cases against
`origin/integration/oss-launch` at commit `4c987b2` ("Darken muted/faint text further and raise
the small-text size floor" — `tools/sopkb` itself last changed at `78b19d0`, "Remove now-dead
legacy Azure-only clients left behind by the LLM unification"), replacing the
byte-identical-to-fork-point corpus jt-dev carried before. Regenerated using a throwaway venv +
checkout of that commit's `tools/sopkb` *outside* this worktree — this worktree's own
`tools/sopkb` (jt-dev's stale, independently-modified copy, out of scope per D1/D4) was never
read or written. The harness's `ENGINES["python"]["cwd"]` is now overridable via the
`SOPKB_HARNESS_PYTHON_CWD` env var (a permanent addition to `harness.py`) specifically to make
this kind of re-baseline possible without touching this checkout's `tools/sopkb`.

*What changed vs. the old fixtures:* 19 of the 20 applicable cases differ
(`malformed-inventory-array` is byte-identical — it fails before the code paths below run). The
dominant driver by far is a **source-versioning subsystem that has landed in oss-launch's
`tools/sopkb` since jt-dev's fork point** — not the PDF/DOCX-specific quirk fixes D1 called out
(character dedup, footer digit corruption, table reading-order, watermark-bleed suppression).
None of these 21 cases exercise PDF or DOCX input at all, so those specific fixes remain
unverified by this corpus; whoever authors PDF/DOCX fixture cases for workstream 1 should expect
to see them there instead. Concretely, for every case that reaches source ingestion:
- Source ids changed from content-hash-suffixed (`glp1-intake-497fafac4587`) to a bare stem plus
  an explicit version id (`glp1-intake`, `source_version_id: glp1-intake:v1`). `manifest.yaml`
  source entries gained `source_version_id` and `status` fields.
- Normalized/original source filenames changed from `<stem>-<hash>.md` to `<stem>.md` /
  `<stem>__v1.md`; section/knowledge/evidence/rule/relation ids changed from `...-<hash>-NNN` to
  `...-v1-NNNNNN` accordingly. This is renaming only — spot-checking `sections.json` confirms
  `start_pos`/`end_pos`/`heading`/`semantic_role` values are unchanged, i.e. the underlying
  markdown segmentation logic itself did not change.
- Two new state files, `.sopkb/source_events.json` (an append-only ingestion event log) and
  `.sopkb/source_versions.json` (a version registry), now appear in every case's bundle —
  including cases that fail early (e.g. `malformed-null-confidence`), confirming they initialize
  unconditionally. A new `reports/source_update_impact.md` report also appears in some cases
  (e.g. `ascii-md`).

Re-verified via `run-all --engine python` reporting `PASS` for all 20 applicable cases (`SKIP`
for `case-only-names` as before) on two consecutive fresh runs, matching this corpus's own "done
when" criterion.

Getting there surfaced four real bugs in the harness itself (not the product), now fixed:
- `REPO_ROOT` was computed one directory level too shallow.
- `@rmdir`'s path token has a placeholder embedded in a larger path expression
  (`{bundle}/.sopkb/cache`), which the exact-match-only token resolver couldn't handle — added
  `resolve_path_expr` for substring substitution.
- The six `\g<pre>...$`-anchored normalization regexes silently failed to match at all on this
  Windows host, because the reference Python CLI writes CRLF-terminated text (default text-mode
  `open(..., "w")` translation) and MULTILINE `$` only matches immediately before `\n`, not before
  a `\r` that precedes it — fixed by anchoring on an explicit, captured, echoed-back `\r?` instead
  of a bare `$`.
- `_transcript.txt` was written without `newline=""`, so captured child stdout that already
  contained literal `\r\n` (from the child process's own Windows text-mode translation) got
  translated a second time into `\r\r\n`, which the `\r?` fix above didn't anticipate — fixed by
  writing the transcript untranslated. Two additional normalization gaps were also found this way:
  the `review *` commands' pretty-printed JSON stdout (captured into `_transcript.txt`) carries its
  own real `"timestamp"` field, invisible to the filename-scoped JSON rule; and `validate_bundle()`
  copies `inventory.json`'s warning paths verbatim into `reports/validation.json`/`.md`, so the
  same P-V7 host-absolute-path leak resurfaces there too, not just in `inventory.json`.

A fifth bug surfaced during the 2026-08-21 re-baseline above, once the new `source_events.json`
state file (see above) started appearing in every case: its `"path"` field leaks the same
ephemeral per-run workdir absolute path the harness already normalizes elsewhere (P-V7-style),
and its `"id"` field embeds a 14-digit timestamp suffix that the existing `RE_JSON_TS`/`RE_WARN_PATH`
rules didn't cover (they key off field name, and this field is named `"id"`, not `"path"` or
`"timestamp"`) — fixed with two new rules, `RE_SOURCE_EVENT_PATH` and `RE_SOURCE_EVENT_ID`.
Without this, `run-all --engine python` could never reach zero-diff no matter how the fixtures
were recorded, since every recording would embed that run's own ephemeral timestamp/path.

**Correction to the re-baseline write-up above, found while implementing workstream 2 (2026-08-22).**
The claim that both new state files "now appear in every case's bundle... confirming they initialize
unconditionally" is too strong, and the fixture data itself is the counter-evidence: `source_versions.json`
appears wherever the source-version *migration* runs (which is every command entry point, hence nearly
everywhere), but `source_events.json` appears only where a *scan* runs. `malformed-null-confidence`, whose
`script` is just `validate {bundle}`, has `source_versions.json` and no `source_events.json`. The Rust port
matches the fixture data rather than the sentence above: the event log is created by the first event, not by
bundle initialization.

**A sixth harness normalization gap, fixed 2026-08-22.** `RE_FRONTMATTER_DATE` covered only `date:`. Source
versioning put the version registry into the `SOP Source` document's YAML frontmatter, including each
version's `modified_time` -- the *input file's* mtime on the recording host, which is exactly as
environment-dependent as `generated.date`, and which `RE_JSON_TS` already normalizes inside `.sopkb/*.json`.
Without the frontmatter counterpart, `sources/*.md` can never reach zero-diff: the recorded document holds
the recording host's mtime while a re-sync of the (normalized) fixture bundle faithfully propagates the
`<TS>` sitting in its own `inventory.json`. The rule now covers `date|modified_time` and allows a leading
`-` in its indent class so it matches a value nested under a YAML sequence item. Applied in both places that
implement this normalization: `harness/harness.py` and its Rust mirror in
`crates/sopkb-export/tests/phase5_v1_diff.rs`.

**`phase9_concept_graph_dump.json` was re-keyed 2026-08-22, not re-recorded.** That dump is captured Python
output for `concept_index`/`focused_graph_for_concept`, and it was captured *before* the re-baseline above,
so every id in it was still content-hash-suffixed. Rather than assume the Rust output was right and record
over it, each id was looked up in the re-baselined bundle's own recorded
`items.json`/`sections.json`/`inventory.json` -- matching on source stem plus ordinal for knowledge ids, and
asserting every substituted result actually exists in that ground truth -- so the expectations still come
from real Python output. 71 knowledge-id, 16 section-id and 16 source-id substitutions; the script asserted
no 12-hex-character suffix survived. Re-recording it properly (via `gen_phase9_concept_graph_dump.py` against
a throwaway oss-launch checkout, the way the corpus itself was re-baselined) is still the better long-term
move if that dump needs to change again for any reason beyond ids.

**Known environment quirk on this host:** if there's a delay between `record` and `run`/`run-all`
(rather than running them back-to-back), some background process on this VM has been observed to
delete freshly-written `expected-python/` files before the verify pass reads them, producing
spurious `SKIP ... no expected-python/ recorded yet` or `only in actual: ...` (missing-file)
results. Not reproduced when the two commands run in immediate succession. If you see this, re-run
`record` immediately before `run-all` rather than assuming the harness or fixtures are wrong.

## PDF cases added 2026-08-22 (workstream 1)

Three PDF cases now exist, closing the "none of the 21 cases exercise PDF or DOCX input at all" gap
noted above for the PDF half:

| Case | What it pins |
| --- | --- |
| `simple-pdf` | Baseline single-column prose: character extraction, word building, line clustering, and the `# <first line>` title promotion. |
| `multipage-gap-pdf` | The G-A19 / P-N15 page-gap behavior. Its middle page has no text operators at all, so the normalized output goes straight from `<!-- page 1 -->` to `<!-- page 3 -->` — the empty page is skipped entirely (no OCR, no placeholder) while the numbering keeps the TRUE 1-based page index. **That gap is correct output, not a dropped page.** |
| `two-column-pdf` | `_detect_column_gutter` / `_columns_from_words`: the whole left column must precede the whole right one rather than being interleaved by height, and a genuinely page-spanning line must not be torn in half at the gutter. |

The `input/sources/*.pdf` files are committed binaries so the corpus stays runnable without Python, but
they are reproducible rather than opaque — `harness/gen_pdf_cases.py` regenerates them from
hand-written content streams via `harness/make_pdf.py` (a dependency-free minimal PDF writer, chosen
over reportlab/fpdf precisely so each fixture can target one specific extraction behavior). Editing a
case's input means re-recording its `expected-python/`.

`expected-python/` for these three was recorded the same way as the 2026-08-21 re-baseline: against a
throwaway extraction of `origin/integration/oss-launch`'s `tools/sopkb` *outside* this worktree, via
`SOPKB_HARNESS_PYTHON_CWD`. This worktree's own `tools/sopkb` was never read or written.

**These cases will not pass `run-all --engine rust` yet, and that is expected** — for the same
workstream-2 reason the other 20 don't: the Rust engine still generates old-scheme
(content-hash-suffixed) source ids while `expected-python/` carries the re-baselined
bare-stem-plus-`source_version_id` scheme. Per CATCHUP_PLAN.md's "Finding, 2026-08-21", workstream 1
owns extraction *content* only, so PDF extraction correctness is verified at that level instead, by
`crates/sopkb-core/tests/pdf_fixture_content.rs`, which diffs `normalize_pdf`'s output against the
recorded `expected-python/bundle/sources/normalized/*.md` byte for byte.

Two further differential harnesses live in `harness/` for the PDF work, both requiring Python only
when deliberately run (never during `cargo test`):
- `diff_pdf_extraction.py` — builds 10 synthetic PDFs and compares every extracted character's
  text/x0/x1/top/bottom/size/upright, plus `extract_text()`, against real pdfplumber.
- `diff_normalize_pdf.py` — compares full `normalize_pdf` output against the real oss-launch Python
  end to end. Needs `SOPKB_OSSLAUNCH` pointing at a directory containing oss-launch's `sopkb` package.

**`golden/` is intentionally still empty.** PORT_PLAN.md §6.0 wants a pristine copy of
`examples/glp1-healthcare/bundle` here, but that bundle currently has a 21-file dev-scratch upload
corpus sitting in `.sopkb/uploads/current/docs/` (unrelated to the 4 canonical GLP-1 sources, added
incidentally in an earlier desktop-port commit) that should be pruned before — or instead of —
copying it into `golden/`. That's a repo-hygiene decision for whoever does this next, not something
to silently paper over by copying it as-is.
