# UI Checklist

Automated checks to run after implementation. Each check maps to a gotcha.

## Static Checks

### S1. Container child manual positioning → G1
Grep `.tscn` files for Controls inside Containers:
- Flag children with `anchor_*`, `offset_*`, or `position` overrides
- Flag content-less Controls (Panel, Control) without `custom_minimum_size`

### S2. Control.scale usage → G2
Grep scripts for `.scale =` or `tween_property.*"scale"` targeting Control-derived nodes:
- Flag any scale manipulation on Controls
- Expected: use `size` or `custom_minimum_size` instead

### S3. z_index input assumption → G3
Grep `.tscn` for Control nodes with non-default `z_index`:
- Verify node does not rely on z_index for input priority
- Must have `mouse_filter = Stop` blocker if modal input needed

### S4. Focus initialization → G4
Grep scripts for `grab_focus()`:
- Every scene with interactive Controls must call `grab_focus()` in `_ready()` or a show signal

### S5. Theme inheritance scope → G5
Grep `.tscn` for `theme = ExtResource` or `theme = SubResource`:
- Flag non-root nodes with Theme resource (potential inheritance cutoff)

### S6. Mouse filter audit → G6
Grep `.tscn` for Panel, ColorRect, TextureRect nodes:
- Non-interactive overlays must have `mouse_filter = 2` (Ignore)
- Flag transparent Controls with default `mouse_filter = 0` (Stop)
- Exception: modal blocker overlays should use Stop intentionally — verify intent, not auto-flag

### S7. ScrollContainer focus boundary → G7
For every ScrollContainer with focusable children:
- Boundary children must have explicit `focus_neighbor_*` to external targets

### S8. Hidden node focus mode → G8
Grep scripts that set `visible = false` on Controls:
- Must also set `focus_mode = FOCUS_NONE` on hidden focusable children
- Also check TabContainer, TabBar, and similar controls that hide children implicitly — inactive tabs retain focus_mode by default

## Runtime Checks

### R1. Zero-size Control test → G1
Load UI scene, iterate all visible Controls:
```
assert(control.size.x > 0 and control.size.y > 0, "zero-size: " + control.name)
```

### R2. Focus test → G4
Load menu scene, simulate Tab:
```
assert(get_viewport().gui_get_focus_owner() != null, "no focus after Tab")
```

### R3. Modal input blocking → G3
Open modal dialog with full-screen Stop blocker, simulate click behind it:
```
assert(behind_button.is_pressed() == false, "click leaked through modal")
```

### R4. Mouse filter transparency test → G6
Place transparent Panel (default mouse_filter) over a Button, simulate click:
```
assert(button.is_pressed() == false, "transparent Panel with Stop consumed click")
panel.mouse_filter = Control.MOUSE_FILTER_IGNORE
simulate_click(button_pos)
assert(button.is_pressed() == true, "click should reach button after Ignore")
```

### R5. ScrollContainer focus escape → G7
Focus last child in ScrollContainer, simulate exit direction:
```
Input.action_press("ui_down")
Input.action_release("ui_down")
await get_tree().process_frame
assert(get_viewport().gui_get_focus_owner() == external_target, "focus trapped")
```

### S9. Control parent type → G9
Grep `.tscn` files for Control-derived nodes (Panel, Label, Button, HBox, VBox, etc.):
- Flag if their parent node is Node2D or Node (not a Control subclass)
- Expected: Control nodes must be under a Control parent for layout to work

### S10. Visual nodes under Entity → G10
Grep `.tscn` and scripts for `add_child()` calls adding CanvasItem nodes (ColorRect, Sprite2D, TextureRect) to Entity nodes:
- Flag if Entity (extends Node) is the parent
- Expected: visual nodes in a separate Node2D/Control subtree, mapped to entities via Dictionary

## Compilation

```bash
{godot_path} --headless --quit 2>&1
```
Pass: exit code 0, no errors referencing UI nodes or scripts.
