# okf.md tools-page submission

**Where:** new issue at https://github.com/fabricioctelles/skills/issues/new
**Route verified:** `okf.md/contribute` names this repo as the submission channel.
Four prior tool submissions there (#4, #5, #6, #7) were all accepted and closed.

**Title:**

```
okf.md Tools page: proposing KL4A (human review gate for extracted claims)
```

---

## Body

Hi Fabricio - thanks for maintaining the Tools page. The "should you bother" tone is
what makes it worth reading, so I've written the limitations properly rather than
glossing them.

I went through the page carefully first, signed-okf and the validators included, since
"verify" covers a fair bit of ground there. What I couldn't find is anything that
records a person judging whether an extracted claim is a faithful reading of its source.

A concrete case from the example bundle in the repo. The miner pulled out:

> Staff must attach diagnosis evidence and prior therapy documentation when required by
> the payer.

Well-formed. The byte range resolves to a real sentence in a real source file. It would
hash, sign and validate without complaint. A reviewer rejected it anyway - the source
makes that conditional, and the extraction reads as a blanket obligation. In a prior-
authorization workflow that's the difference between "sometimes" and "always".

That judgement, and the reason behind it, is what KL4A stores. If something on the page
already does this and I've misread it, tell me and I'll withdraw.

Entry below in your format - edit or downgrade as you see fit.

---

### KL4A - Knowledge Layer For Agents

**What it is:** A producer for SOPs, policies and regulations that puts a human in front
of `verified`. It pulls obligation-shaped claims out of prose, ties each one to the byte
range of the sentence it came from, and won't mark anything verified until a person has
approved, rejected, deferred or edited it - with their name and a written reason attached.

**How it works:** `scan` inventories and checksums the sources (PDF, DOCX, Markdown,
text), `normalize` splits them into sections, `mine` proposes claims, `review` records
what a human decided, `export` writes the OKF documents plus graph JSON and RDF. The
originals stay in the bundle verbatim under `sources/originals/` next to the derived
text, so a byte-range citation is still checkable by whoever you hand the bundle to.
Everything is plain files - no database, nothing to stand up. A read-only MCP server
serves the bundle to agents and gives them a grounding contract before they've asked
anything.

**Stack:** Rust - 10 crates, a CLI, an MCP server and an HTTP server - plus a Tauri
desktop app for Windows, macOS and Linux. Apache-2.0.

**Links:** https://github.com/CogniSwitch/KL4A

**Try it:** A full worked example ships in the repo - `examples/glp1-healthcare/`
carries the source documents, the review decisions and the exported OKF bundle, so you
can read the whole pipeline's output without installing anything. Or point it at your
own PDFs, DOCX or Markdown and run it end to end.

**Limitations:**

- It's 0.0.2, and the desktop builds aren't code-signed yet, so macOS and Windows
  both complain on first launch.
- A conformance defect I'd rather flag than have you find: we write
  `generated: {actor, date}` and `verified: [{actor, date}]` where §5.2 says `{by, at}`,
  and `verified[].at` is null right now. Bundles are otherwise v0.2-shaped and validate.
  Fix is in progress.
- PDF and DOCX now ingest and normalize everywhere - desktop, CLI and server. But the
  offline miner won't read a sentence that's broken across lines, and PDFs keep the page's
  wrapping, so on a typical PDF most sentences never reach it. Give PDFs the LLM miner or
  expect a thin bundle.
- Evidence is a byte range into text, so there's no way to cite into an image or a video
  timestamp.

**Maturity:** 🔴 pre-alpha - the pipeline, the review gate and the MCP server work end to
end, but it's 0.0.1, there's a known conformance gap, and there's exactly one producer.
Slot it wherever that lands on your scale.

---

Happy to rework any of this.

---
---

## Notes for whoever posts this (not part of the submission)

**Format matched to what actually gets accepted.** The `/contribute` page prescribes a
`[OKF Tool] Name` title and a field list, but none of the four accepted submissions used
it. They wrote descriptive titles and drafted the entry in the Tools page's own format -
What it is / How it works / Stack / Links / Limitations / Maturity. This follows the
accepted pattern rather than the documented one.

**The maintainer stated his acceptance criteria** in his reply to okf-guard (#6):
"The tool fills a real gap - nobody else was doing pre-generation content safety for OKF
pipelines. Good documentation, honest limitations, solid implementation."

**Maturity is deliberately under-claimed.** AIX (#4) self-assessed 🟡 with a spec,
validator and one example bundle, and offered to be downgraded. 🔴 plus an explicit
invitation to downgrade further is the safer side with this curator.

**Two accuracy caveats if he replies:**

1. The rejected claim comes from the GLP-1 example bundle. Its source documents are
   fabricated - the example's own README says so in bold on line 3, which is why the
   limitations list no longer repeats it. The review itself was performed for real. If he
   asks, it illustrates a class of error; it is not a finding from a customer corpus.
0. The PDF limitation is two separate defects. The feature-flag one is fixed and merged
   (PR #3, 2026-09-09) - that's why the bullet no longer says "in review". The line-break
   one in the offline miner is not fixed and is what the bullet now describes. Don't let a
   reply collapse them into one.
2. "Would hash, sign and validate without complaint" is accurate and checked against
   signed-okf's SPEC.md - its signature covers whole-bundle file hashes plus an issuer
   string, and its own Semantics section says it "does not assert that the content is
   true". It is not a criticism of that tool, and shouldn't be reworded into one.

**Do not** rewrite the opening to say "I'd rather give you accurate limitations than a
sales pitch" - that is near-verbatim from the okf-guard submission to the same person.
