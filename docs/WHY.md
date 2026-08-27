# Why We Built This

## Agents need somewhere to get the rules from

That agents need a knowledge layer is no longer the interesting question. Which
layer is the right one still is, and nobody has the full answer yet.

What organizations have today is knowledge written for people. SOPs, policies,
procedures, regulations and standards live as Word documents and PDFs, sitting
in folders on Google Drive, on SharePoint, or on somebody's laptop. Source code
lives the same way, in folders on GitHub. That folder is the structure everyone
already uses, and it works.

Most current tooling starts from a different assumption: foundation models are
strong at natural language, so you can point one at a natural-language source
and let it work things out. For anything that has to be followed exactly, that
assumption breaks down. Ask an agent to apply a procedure and you often get an
answer that reads well and is wrong in a way nobody catches until it matters.

## What this project does about it

This is a transformation, not a new system to adopt.

Your documents stay where they are. The workbench reads them, proposes
structured knowledge from what it finds, and puts a person in front of every
proposal before anything is accepted. What comes out is a set of plain files in
a folder, next to the documents they came from. There is no database and nothing
to migrate onto. It is a small change to your setup and a large change in how an
agent behaves against the same material.

Two properties carry the weight.

**Every claim keeps the text it came from.** A knowledge item is not a summary
that has floated free of its source. It carries the exact span it was derived
from, or it is flagged when that span could not be matched. When an agent
answers from a bundle, you can follow the answer back to the sentence in the
original document.

**Nothing is usable until a person approves it.** Extraction proposes. A
reviewer approves, rejects, defers, edits or comments, and that decision is
recorded in the bundle along with who made it and why. This is not a rubber
stamp over runtime output. It happens once, over a bounded set of claims, before
anything goes into service.

## Why it is open source

A bundle is only trustworthy if you can read it without us. The format is plain
Markdown with YAML frontmatter, so a text editor or `grep` is enough to inspect
what an agent is being told. A claim about provenance that you cannot verify
yourself is not much of a claim.

Keeping bundle creation and review open also keeps us honest about where the
commercial line sits — see [Governance](GOVERNANCE.md) for exactly where that boundary is
drawn and why.

---

# What We Think of OKF

OKF, the Open Knowledge Format, is an open standard published by Google Cloud
for representing knowledge as Markdown files with YAML frontmatter, organized in
directories. We did not invent it and we are not trying to replace it.

## Its minimalism is the point

OKF asks for very little. A small set of required keys is enough for
interoperability, and beyond that a bundle may carry additional frontmatter keys
and body sections without breaking anything that consumes it.

That restraint is the most attractive thing about it. A standard that specifies
everything up front tends to specify the wrong things, because the people
writing it are guessing about uses that do not exist yet. A standard that
specifies the minimum leaves room for practice to decide.

There is an obvious risk, which is fragmentation: everyone extends in their own
direction and interoperability becomes theoretical. Our expectation runs the
other way. As more people build against it, agreement should form about what is
genuinely needed, and more of it should become required. We would rather take
part in that than wait for it to settle.

## A conformant bundle is not automatically a useful one

Meeting the format and being useful to an agent are different achievements.
Google's own repository ships reference implementations and worked example
bundles alongside the specification, which suggests they see the same gap.

Our work sits in that gap. We stay inside the format and add depth: evidence
spans, review state with reviewer rationale, decision rules that can be checked
mechanically, and conflict and freshness reporting. All of it lives under our
own namespace, as the standard's extension rules allow, so a bundle we produce
stays readable by any OKF consumer that has never heard of us.

Whether we have added the *right* depth is genuinely open. Some of it may turn
out to be more than anyone needs, and some of it may be too thin. That gets
settled by people using it, not by us asserting it.

## What we are betting on

Version 0.2 introduced fields concerned with provenance and trust. Those are the
fields we have built around, because the problem we care about is not retrieving
relevant text. It is being able to say where a claim came from and who checked
it.

If you are comparing this against other OKF tooling, that emphasis is the
difference worth looking at.

## What interoperability buys

The reason to stay inside a shared format rather than invent our own is
straightforward. If the structure is the same everywhere, one organization's
agent can read another organization's knowledge without a bespoke integration
for every pair. Enterprises will need access control and exposure rules layered
on top, and those are their decisions to make. The path has to exist first.

## Where we would like help

A few questions we would rather answer with the community than on our own:

- What should become required in a future revision of the format?
- Do decision rules belong in the format itself, or in a layer above it?
- How should confidence be expressed? Ours is currently a placeholder and we
  know it.
- Which parts of our extension are worth proposing upstream?

We would rather be told we got this wrong early than be right in private.
