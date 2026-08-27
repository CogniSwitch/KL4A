# Sample Agent Queries

Run from `tools/sopkb`:

```powershell
python -m sopkb.cli bundle describe ..\..\examples\glp1-healthcare\bundle
python -m sopkb.cli sources list ..\..\examples\glp1-healthcare\bundle
python -m sopkb.cli knowledge search ..\..\examples\glp1-healthcare\bundle "contraindications"
python -m sopkb.cli knowledge search ..\..\examples\glp1-healthcare\bundle "prior authorization"
python -m sopkb.cli conflicts list ..\..\examples\glp1-healthcare\bundle
python -m sopkb.cli freshness check ..\..\examples\glp1-healthcare\bundle
```

