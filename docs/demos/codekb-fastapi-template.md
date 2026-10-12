---
title: CodeKB Demo — FastAPI Full-Stack Template
description: Evaluate repository structure, task context, and agent-grounded change planning against a real Python application.
---

# CodeKB Demo: FastAPI Full-Stack Template

This demo uses a real, version-pinned application template to test whether a
developer or agent receives useful, grounded code context before proposing a
change. It is not a claim that CodeKB analyzes every language in the repository.

## Fixed input

- Repository: [fastapi/full-stack-fastapi-template](https://github.com/fastapi/full-stack-fastapi-template)
- Release: `0.9.0`
- Commit: `e4022a9502a6b61c857e3cbdaddc69e7219c9d53`
- GitHub source archive SHA-256: `F7AB9220D25C804623677877D621D7C945CA7A78AAAC31D66631B6EA759DB23B`

Clone the exact tag, then verify the resolved commit before building:

```powershell
git clone --depth 1 --branch 0.9.0 https://github.com/fastapi/full-stack-fastapi-template.git demo-code\repo
git -C demo-code\repo rev-parse HEAD
```

The output must be the commit above. Record it in the evaluation notes.

## Prerequisites

Use the released Windows binary directly:

```powershell
$kl4a = (Resolve-Path .\kl4a-windows-x86_64.exe)
& $kl4a --version
```

On macOS or Linux, use the matching `kl4a` release asset and make it executable
before substituting its path. This demo deliberately uses Python because the
current CodeKB parser supports Python and COBOL.

## Run

Build deterministically and offline first:

```powershell
& $kl4a --use codekb build demo-code\repo --bundle demo-code\bundle --mining static
& $kl4a --use codekb validate demo-code\bundle
& $kl4a --use codekb repo describe demo-code\bundle
& $kl4a --use codekb context demo-code\bundle --task plan-change --query "user creation"
```

Save the JSON returned by `repo describe` and `context`. The context response
should identify matched symbols, evidence excerpts, relations, tests, and any
warnings. Do not judge success by an assumed symbol count; use the bundle's
actual output.

## Agent task and evidence requirements

Register the read-only MCP server with an MCP-capable agent:

```json
{
  "mcpServers": {
    "codekb-demo": {
      "command": "C:\\absolute\\path\\to\\kl4a-windows-x86_64.exe",
      "args": ["--use", "codekb", "mcp", "serve", "C:\\absolute\\path\\to\\demo-code\\bundle"]
    }
  }
}
```

Give the agent this bounded task:

> Plan a change affecting user creation. Identify the relevant endpoint, request
> schema, persistence/model path, direct dependencies, and tests. Do not write
> code. State every uncertainty and cite the CodeKB evidence for each component.

The agent must call `code.context` first, then use `code.symbols.search`,
`code.change_impact`, or `code.tests.for_symbol` as needed. Its plan is valid
only if each named component appears in the returned tool evidence. A human
evaluator should open three cited excerpts in the cloned repository and compare
them with the plan.

## Evaluator checklist

- [ ] The clone resolves to the pinned commit.
- [ ] `validate` reports no errors; warnings are recorded rather than discarded.
- [ ] The bundle describes Python architecture relevant to the task, where the
      source contains it (for example routes, schemas, models, or dependencies).
- [ ] The agent used the MCP tools before writing its plan.
- [ ] Three sampled citations match the repository source.
- [ ] The plan separates verified facts from warnings or unknowns.
- [ ] No claim is made about unparsed frontend code.

## Limitations

- The repository also contains non-Python material. CodeKB currently parses
  Python and COBOL, so TypeScript/frontend behavior is outside this evaluation.
- `--mining static` is intentional: it is repeatable and offline, but can yield
  thinner claims than a reviewed hybrid-mining run.
- Unresolved external relations are warnings, not validation errors. They mark
  a boundary of the repository evidence rather than a defect to conceal.
- The MCP server is read-only; it cannot approve claims or alter the repository.
