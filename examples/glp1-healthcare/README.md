# GLP-1 Healthcare SOP Reference Bundle

> **Disclaimer: synthetic data only.** Every document, policy, procedure, patient scenario, name, and identifier in this example is **fabricated** for demonstration purposes. Nothing here describes a real patient, clinician, organization, or clinical case, and no real personally identifiable information (PII) or protected health information (PHI) is included anywhere in this bundle. Do not treat any content in this example as clinical, legal, or regulatory guidance.

This example is intended for demos, tests, and local development.

It demonstrates:

- multiple SOP-like source documents,
- SOP vs policy vs workflow differences,
- markdown source normalization,
- evidence-backed proposed knowledge,
- persisted HITL review states,
- conflict and freshness report examples,
- OKF, graph JSON, and RDF/TTL exports,
- local agent query examples.

The bundle under `bundle/` is checked in already built — there's nothing to
generate before you can use it. Its markdown authoring sources live in
`sources/`.

## Opening it

The simplest way in is the desktop app: launch **KL4A Workbench** and, on the
bundle picker, point it at `examples/glp1-healthcare/bundle`. See the
[Quickstart](../../docs/quickstart.md) for install links.

To query it from the command line instead, see
[`agent_queries.md`](agent_queries.md).

To serve the same UI in a browser, run the `sopkb-server` binary — it prints a
bearer token and a pre-filled URL on startup:

```bash
sopkb-server --bundle-dir examples/glp1-healthcare/bundle
```

It binds `127.0.0.1:4173` by default; override with `--bind`.

To point an agent at the bundle over MCP, see
[`docs/MCP_SERVER.md`](../../docs/MCP_SERVER.md).
