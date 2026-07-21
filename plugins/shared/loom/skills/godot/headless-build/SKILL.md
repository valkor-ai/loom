---
name: headless-build
description: |
  Compile-check a Godot project using headless mode.
  Use after writing or modifying any GDScript or C# code to verify it compiles.
  Triggers: after code changes, before running tests, when build errors need diagnosis.
  Also use this when the user says "check if it compiles", "build check", "does this parse",
  or after any file creation/modification that could break compilation.
---

# Headless Build

Compile-check a Godot project. Fastest feedback loop: "did my code parse?"

## Run

Use `GODOT_PATH` when Godot is not on `PATH`:

```bash
GODOT_BIN="${GODOT_PATH:-godot}"
"$GODOT_BIN" --headless --path . --editor --quit 2>&1
```

Run from the project root (where `project.godot` lives). Should exit within 30 seconds.
If it hangs, an autoload may block in `_ready()` — kill and report.

## Filter output

Godot's output mixes script errors with engine noise. Only script errors matter.

**Ignore these** (engine internals, always present in headless mode):

- `Condition "..." is true` with `.cpp` source location — engine C++ assertion
- `ObjectDB instances leaked at exit` — no cleanup in headless
- `N resources still in use at exit` — same cause
- `Screen index N is invalid` — no display in headless

**Rule**: if the `at:` line references a `.cpp` file, skip it.

**These ARE script errors** (report them):

- `SCRIPT ERROR:` with `res://` path — GDScript parse/load failure
- `Failed to load script "res://..."` — missing or broken script
- `error CS` — C# build error (from dotnet build)
- `Failed to load resource "res://..."` — missing dependency

## Report

- No script errors → **PASS**
- Script errors found → **FAIL**, list each with file, line, message
- Don't auto-fix. Report only.
- Ignore `WARNING:` lines and application log output.
