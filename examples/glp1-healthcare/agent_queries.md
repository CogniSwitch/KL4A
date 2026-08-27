# Sample Agent Queries

Run from the repository root, with `sopkb-cli` on your `PATH` (or substitute the
path to the built binary, e.g. `v2/sopkb-rust/target/release/sopkb-cli`):

```bash
sopkb-cli bundle describe examples/glp1-healthcare/bundle
sopkb-cli sources list examples/glp1-healthcare/bundle
sopkb-cli knowledge search examples/glp1-healthcare/bundle "contraindications"
sopkb-cli knowledge search examples/glp1-healthcare/bundle "prior authorization"
sopkb-cli conflicts list examples/glp1-healthcare/bundle
sopkb-cli freshness check examples/glp1-healthcare/bundle
```

Captured output for these queries is checked in as
[`sample_agent_query_results.json`](sample_agent_query_results.json).
