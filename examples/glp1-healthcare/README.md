# GLP-1 Healthcare SOP Reference Bundle

> **Disclaimer: synthetic data only.** Every document, policy, procedure, patient scenario, name, and identifier in this example is **fabricated** for demonstration purposes. Nothing here describes a real patient, clinician, organization, or clinical case, and no real personally identifiable information (PII) or protected health information (PHI) is included anywhere in this bundle. Do not treat any content in this example as clinical, legal, or regulatory guidance.

This example is intended for demos, tests, and local development.

It demonstrates:

- multiple SOP-like source documents,
- generated DOCX and PDF input sources,
- SOP vs policy vs workflow differences,
- markdown source normalization,
- evidence-backed proposed knowledge,
- persisted HITL review states,
- conflict and freshness report examples,
- OKF, graph JSON, and RDF/TTL exports,
- local agent query examples.

Rebuild from the repository root:

```powershell
python examples\glp1-healthcare\rebuild_reference_bundle.py
```

The rebuild script creates `generated_sources/` from the markdown authoring sources so the bundle demonstrates markdown, DOCX, and PDF ingestion without requiring hand-maintained binary source files.

Then launch the local workbench:

```powershell
cd tools\sopkb
python -m sopkb.cli serve ..\..\examples\glp1-healthcare\bundle --port 8765
```

Open:

```text
http://127.0.0.1:8765
```
