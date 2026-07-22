---
name: godot
description: |
  Build, test, debug, and visually verify Godot projects through Loom.
  Use this skill for Godot project structure, GDScript or C# API questions,
  headless checks, input maps, runtime inspection through godot-mcp, gameplay
  tests, screenshots, and handoff evidence.
---

# Godot Skills for Loom

Loom provides the delivery workflow. These references provide Godot-specific
working knowledge; they do not replace the Godot editor, the Godot runtime, or
the project's own addons.

## Choose the smallest skill

| Task | Skill |
| --- | --- |
| Godot API or GDScript/C# syntax | `godot-api` |
| Parse or compile check | `headless-build` |
| gdUnit4 tests | `gdunit-driver` |
| Live runtime, scene tree, console, or input debugging | `mcp-driver` |
| Project layout or `project.godot` settings | `project-scaffold` |
| Input actions | `input-mapper` |
| End-to-end gameplay or UI tests | `godot-e2e` |
| Capture a gameplay frame | `screenshot` |
| Inspect a capture for acceptance issues | `visual-qa` |
| GDScript linting and formatting | `gdtoolkit` |
| Animation, audio, navigation, particles, physics, shaders, TileMap, or UI review | `reviewer/*` |

Do not load every reference for every task. Start with `mcp-driver` only when
the problem is runtime-specific; use `headless-build` for syntax and import
failures first.

## Loom delivery loop

For a non-trivial Godot change, route the work through Loom before editing:

```text
@loom plan Add a third-person dog that walks beside the house in the Godot project
@loom continue
```

Keep the Godot project root as the active workspace, with `project.godot` at
the root. Let Loom own the delivery state in `.loom/`; keep screenshots, logs,
and other evidence at the task paths returned by Loom or in the project's
existing evidence directory.

The normal verification order is:

1. Check the Godot version and project structure.
2. Run a headless parse/import check after script or scene edits.
3. Run unit or end-to-end tests when the project has them.
4. Use `godot-mcp` for live behavior and visual inspection.
5. Capture the result and record the evidence in the Loom task handoff.

Specialist reviewers under `reviewer/` run after implementation. Use only the
reviewer that matches the changed Godot subsystem; they supplement Loom review
with engine-specific gotchas and do not replace the Loom review or repair loop.

## Runtime requirements

The skills use `GODOT_PATH` when Godot is not on `PATH`:

```bash
export GODOT_PATH="/Applications/Godot.app/Contents/MacOS/Godot"
"$GODOT_PATH" --version
```

On macOS, the app may live anywhere. Point `GODOT_PATH` at the executable
inside the app bundle, not at the `.app` directory itself. For a standard PATH
installation, `godot` is also accepted.

`godot-mcp` is a separate MCP server. Register it as `godot` in the active
coding agent before using `mcp-driver`; Loom's own MCP server does not provide
Godot runtime tools.
