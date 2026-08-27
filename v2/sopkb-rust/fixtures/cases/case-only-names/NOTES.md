This case's `input/sources/` cannot be authored on a case-insensitive filesystem (default
Windows/macOS) — `Notes.md` and `notes.md` collide into one directory entry there, so this
repo's checkout (Windows) cannot hold both simultaneously. `harness.py`'s `filesystem_is_case_sensitive()`
probe already skips this case on such hosts (see harness.py and PORT_PLAN.md P-I8/DECISIONS.md Q3).

To materialize `input/sources/` on a case-sensitive filesystem (Linux/macOS, or a Windows
volume with per-directory case sensitivity enabled via `fsutil.exe file setCaseSensitiveInfo`),
create exactly these two files:

`input/sources/Notes.md`:
```markdown
# Patient Notes

Staff must confirm identity using the primary intake form.
```

`input/sources/notes.md`:
```markdown
# Patient Notes Addendum

Staff should record any deviations and route exceptions to a supervisor.
```

Then `record`/`run` this case as usual, on that host, with `--engine python`.
