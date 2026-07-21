---
name: mcp-driver
description: |
  Runtime debugging and live project inspection via godot-mcp.
  Use when headless tools cannot diagnose the problem: code compiles but runtime behavior
  is wrong, tests pass but the game does not work as expected, or you need to inspect
  live state (node tree, console output, rendering).
  Escalation trigger: a task has failed 2+ times through the headless path (headless-build,
  gdunit-driver), or the issue is inherently runtime-only (visual glitches, input not
  responding, physics behaving unexpectedly at runtime).
  DO NOT use for: syntax/compile errors (use headless-build), unit test failures
  (use gdunit-driver), API lookups (use godot-api).
  Requires: godot-mcp MCP server registered as "godot" (@coding-solo/godot-mcp).
---

# MCP Driver — Runtime Debug & Inspection

$ARGUMENTS

## When to use this skill

MCP is the **upgrade path** when faster tools hit a wall. The fast loop handles most issues:

| Problem type | Fast path (no MCP) |
|---|---|
| Syntax / parse error | headless-build |
| Unit test failure | gdunit-driver |
| API lookup | godot-api |

Escalate to MCP when:

- **Compile-pass / runtime-fail** — headless-build says OK but something is wrong when the game actually runs
- **Test-pass / behavior-wrong** — unit tests pass but the game does not behave correctly in practice
- **Visual issue** — screen is black, sprites missing, z-order wrong, camera not following
- **Input not working** — keys/mouse/gamepad produce no response at runtime
- **Physics misbehavior at runtime** — objects pass through each other, wrong gravity, jittery movement
- **Need live state** — want to see the actual node tree, console output, or runtime errors
- **Repeated headless failure** — a fix has been attempted 2+ times via headless path without success

If none of these apply, use the fast path skill instead.

## Debug-fix loop (overview)

When you escalate to MCP, follow this loop. Max **3 iterations** before asking the user for help.

```
1. RUN      — run_project (optionally with a specific scene)
2. CAPTURE  — get_debug_output → collect errors, warnings, prints
3. ANALYZE  — parse output, identify root cause
4. FIX      — modify code / scene / config to address the issue
5. VERIFY   — stop_project → run_project again, get_debug_output
6. REPORT   — summarize what was wrong, what was changed, and the outcome
```

Each step is detailed in `references/workflow.md`. Common runtime issue patterns are in `references/debug-patterns.md`.

## Project path

MCP tools require a `projectPath` parameter — the directory containing `project.godot`. Determine it from the current working directory or ask the user if ambiguous:

```bash
# Quick check
ls project.godot 2>/dev/null && echo "Project root: $(pwd)"
```

## Tool quick reference

Full tool documentation: `references/tools.md`

### Debug tools (primary use)

| Tool | Purpose |
|---|---|
| `run_project` | Launch project in debug mode, capture output |
| `get_debug_output` | Retrieve console output and errors from running project |
| `stop_project` | Stop the running project, get final output |

### Inspection tools

| Tool | Purpose |
|---|---|
| `get_project_info` | Analyze project structure (scene/script/asset counts) |
| `get_godot_version` | Check installed Godot version |
| `list_projects` | Find Godot projects in a directory |
| `launch_editor` | Open the Godot editor (for manual inspection) |

### Scene management tools

| Tool | Purpose |
|---|---|
| `create_scene` | Create new scene file |
| `add_node` | Add node to existing scene |
| `load_sprite` | Load texture into Sprite2D node |
| `save_scene` | Save/resave scene file |
| `export_mesh_library` | Export scene as MeshLibrary |

### UID tools (Godot 4.4+)

| Tool | Purpose |
|---|---|
| `get_uid` | Get UID for a specific file |
| `update_project_uids` | Resave all resources to update UID references |

## Iteration budget

| Iteration | Action |
|---|---|
| 1 | Run → capture → analyze → apply most likely fix → verify |
| 2 | Re-analyze output, try alternative hypothesis → verify |
| 3 | Broaden investigation (check project structure, cross-reference with reviewer skills) → verify |
| After 3 | **Stop and ask the user.** Report what was tried and what was observed. |

## Cross-references

After MCP reveals the root cause, hand off to the appropriate skill for the fix:

- Physics issues → physics reviewer skill
- Animation issues → animation reviewer skill
- UI layout issues → ui reviewer skill
- Shader/rendering → shader reviewer skill
- ECS entity problems → gecs skill
