# Debug-Fix Loop Workflow

Detailed step-by-step workflow for runtime debugging via godot-mcp.

## Prerequisites

Before entering this workflow, confirm:
1. The issue is NOT a compile error (use headless-build) or test failure (use gdunit-driver)
2. You know the project path (directory containing `project.godot`)
3. The godot-mcp server is registered and accessible

## The loop

### Step 1: RUN

Launch the project via MCP.

```
Tool: run_project
  projectPath: "<absolute path to project>"
  scene: "<optional: specific scene to test>"
```

**Decision — which scene to run:**
- If the issue is in a specific scene, pass it via `scene` to skip menus/loading
- If you don't know which scene has the problem, omit `scene` to run the default main scene
- If there's no main scene set, use `get_project_info` first to find available scenes

**What to expect:** A confirmation message. The project is now running in the background.

### Step 2: CAPTURE

Wait briefly for the project to initialize, then capture output.

```
Tool: get_debug_output
  (no parameters)
```

**Timing:** Wait 2-3 seconds after run_project before calling this. For games with loading screens or complex initialization, you may need to wait longer or call multiple times.

**What you get:**
- `output` array — stdout lines (prints, engine info, warnings)
- `errors` array — stderr lines (script errors, engine errors)

**Priority:** Read `errors` first — these are the most likely clues.

### Step 3: ANALYZE

Parse the captured output to identify the root cause. Follow this priority order:

**Priority 1 — Script errors:**
Look for `SCRIPT ERROR:`, `Failed to load script`, `error CS` (C#), or `Failed to load resource`. These give you a file path and line number — go there.

**Priority 2 — Runtime errors:**
Look for `Invalid get index`, `null instance`, `Invalid type`, `Out of bounds`. These indicate logic errors that only appear at runtime.

**Priority 3 — Warnings:**
Look for lines containing `WARNING:` — these are non-fatal but may explain unexpected behavior (deprecated functions, missing resources, etc.).

**Priority 4 — Absence of expected output:**
If the game should print something (debug prints, state changes) and doesn't, that's a clue too — the code path isn't being reached.

**Priority 5 — No errors at all:**
If there are no errors and no unexpected output but the behavior is still wrong, the issue is likely:
- Visual (camera, visibility, z-order) — can't diagnose from console alone
- Logic (wrong values, wrong branch) — needs debug prints added
- Configuration (project settings, input map) — check project.godot

### Step 4: FIX

Based on the analysis, modify the code, scene, or configuration. See `debug-patterns.md` for common patterns and their fixes.

**Guidelines:**
- Make one fix at a time — don't change multiple things simultaneously, or you won't know what worked
- If the root cause points to a specific Godot subsystem (physics, animation, UI), consult the corresponding reviewer skill for gotcha-aware guidance
- If the fix requires adding debug prints to narrow down the issue, that's fine — just remove them afterward
- For scene-level fixes, you can use MCP scene tools (add_node, load_sprite) or edit the .tscn file directly

### Step 5: VERIFY

Stop the current run and re-launch to test the fix.

```
Tool: stop_project
  (no parameters)

Tool: run_project
  projectPath: "<same path>"
  scene: "<same scene if applicable>"

(wait 2-3 seconds)

Tool: get_debug_output
  (no parameters)
```

**Evaluation:**
- **Fixed** — the original error/behavior is resolved and no new errors appeared → proceed to Step 6
- **Partially fixed** — original error gone but new issues appeared → loop back to Step 3 with the new output
- **Not fixed** — same error persists → loop back to Step 3, try a different hypothesis
- **Worse** — more errors than before → revert the change, loop back to Step 3

### Step 6: REPORT

After a successful fix (or after reaching the iteration limit), summarize:

```
## MCP Debug Report

**Issue:** [what was wrong]
**Root cause:** [why it happened]
**Fix applied:** [what was changed, with file paths]
**Verification:** [confirm the fix works — or explain why iteration limit was reached]
**Iterations:** [how many run-capture-fix cycles it took]
```

Always include file paths and line numbers so the user can review the changes.

## Iteration limits

| Iteration | What to do |
|---|---|
| 1 | Apply the most likely fix based on the error output |
| 2 | If first fix didn't work, re-analyze — consider alternative hypotheses |
| 3 | Broaden the investigation: check project structure, inspect related files, consult reviewer skills |
| **After 3** | **Stop.** Report what you've tried and observed. Ask the user what to try next. |

Three failed iterations usually means:
- The root cause is not what you think it is — you need human insight
- The issue involves external factors (hardware, OS, Godot version bug) the agent can't control
- The problem requires visual inspection that MCP console output can't provide

## Decision tree: when to exit MCP and use other tools

During the debug-fix loop, you may discover the issue belongs to a different tool:

- Error is a syntax/parse error you missed → **stop_project**, switch to **headless-build**
- Fix needs a test to verify a specific function → write test, switch to **gdunit-driver**
- Error references an API you're unsure about → pause, query **godot-api**, then continue
- Issue involves a specific subsystem gotcha → consult the relevant **reviewer skill**, then continue

Switching to another tool doesn't reset your iteration count. If you've already used 2 iterations and need gdunit-driver for one step, that's fine — your MCP budget is about the run-capture-fix cycle, not about using any tool.

## Example session

**Scenario:** Player character doesn't move when pressing arrow keys.

```
Iteration 1:
  RUN     → run_project with projectPath
  CAPTURE → get_debug_output
            output: ["Godot Engine v4.4.stable", ...]
            errors: []              ← no errors!
  ANALYZE → No errors. Input issue suspected. Check code for action names.
            Found: Input.is_action_pressed("move_right")
            Check project.godot: no [input] section → actions not defined!
  FIX     → Add input actions to project.godot:
            [input]
            move_right={ "events": [InputEventKey(keycode=4194321)] }
            move_left={ "events": [InputEventKey(keycode=4194319)] }
            ...
  VERIFY  → stop_project → run_project → get_debug_output
            Player now moves on arrow key press.
  REPORT  → Root cause: Input Map actions were not defined in project.godot.
            Fix: Added move_right/left/up/down actions mapped to arrow keys.
            Iterations: 1
```
