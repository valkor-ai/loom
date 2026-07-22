---
name: input-mapper
description: |
  Manage Godot input action mappings in project.godot.
  Use when adding input actions, changing key bindings, setting up controls
  for a new genre, or validating that all referenced actions exist.
  Use it while implementing mechanics or changing the project's controls.
---

# Input Mapper

$ARGUMENTS

Read the GDD or user request, determine which input actions are needed,
and write the correct `[input]` section entries into `project.godot`.

## Step 1 — Extract Required Actions

Source actions from one of:

1. **GDD Mechanics table** — each mechanic implies input actions
   (e.g., "Player can jump" → `jump` action).
2. **Genre preset** — use a preset from Step 3 as a starting point.
3. **Explicit request** — user or dispatching role specifies exact actions.

For each action, determine: action name (snake_case), bound keys/buttons.

## Step 2 — Write to project.godot [input] Section

For Object() serialization format, templates (keyboard, mouse, gamepad button, gamepad axis), and key code table, read `references/serialization.md`.

## Step 3 — Genre Presets

Use these as starting points. Add gamepad bindings if the GDD mentions controller support.

### Platformer

| Action | Keys |
|--------|------|
| `move_left` | A, Left Arrow |
| `move_right` | D, Right Arrow |
| `jump` | Space, W, Up Arrow |

### Top-down

| Action | Keys |
|--------|------|
| `move_up` | W, Up Arrow |
| `move_down` | S, Down Arrow |
| `move_left` | A, Left Arrow |
| `move_right` | D, Right Arrow |

### Shooter (top-down or twin-stick)

| Action | Keys / Buttons |
|--------|---------------|
| `move_up` | W, Up Arrow |
| `move_down` | S, Down Arrow |
| `move_left` | A, Left Arrow |
| `move_right` | D, Right Arrow |
| `shoot` | LMB (mouse button 1) |
| `reload` | R |

### UI (add to all genres)

| Action | Keys |
|--------|------|
| `pause` | Escape |

Note: `ui_accept`, `ui_cancel`, `ui_left`, `ui_right`, `ui_up`, `ui_down` are
built-in Godot actions — do NOT redefine them in `[input]` unless overriding defaults.

## Step 4 — Validation

Check that every `Input.is_action_*("action_name")` and `event.is_action("action_name")`
call in `.gd` files has a matching entry in `project.godot [input]` or is a built-in
Godot action (prefixed with `ui_`).

To validate:

1. Grep all `.gd` files for `is_action\w*\("([^"]+)"\)` — extract action names.
2. Parse `project.godot` `[input]` section for defined actions.
3. Built-in actions (do not need definition): `ui_accept`, `ui_cancel`, `ui_select`,
   `ui_left`, `ui_right`, `ui_up`, `ui_down`, `ui_focus_next`, `ui_focus_prev`,
   `ui_page_up`, `ui_page_down`, `ui_home`, `ui_end`, `ui_text_*`, `ui_filedialog_*`,
   `ui_graph_*`, `ui_copy`, `ui_cut`, `ui_paste`, `ui_undo`, `ui_redo`.
4. Report any action referenced in code but missing from project.godot.
