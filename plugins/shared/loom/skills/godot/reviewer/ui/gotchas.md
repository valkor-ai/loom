# UI Gotchas

Design-level constraints — intentional engine behaviors, not bugs. Stable across Godot 4.x.

## G1. Container children ignore manual positioning; content-less Controls collapse to 0×0 [GDScript] [C#]

**Symptom**: setting position/anchor/offset/scale on a Container child has no effect. Empty Panels or bare Controls shrink to zero size.

**Root cause**: Containers take full ownership of child transforms on every layout pass. `custom_minimum_size` is the only hard floor — without it, content-less Controls report (0,0). Expand flag only distributes surplus space, cannot create space from nothing.

**Correct approach**: use Size Flags + `custom_minimum_size` for placement. Always set `custom_minimum_size` on Controls that lack intrinsic content (text/icons). For per-child offsets, nest a MarginContainer.

**Wrong approach**: setting `position`, `anchor_*`, `offset_*`, or `scale` on Container children. Using Expand alone without `custom_minimum_size`. Adding an empty Panel to a VBox expecting it to take space.

## G2. Control.scale is post-layout — breaks sizing and hit-testing [GDScript] [C#]

**Symptom**: scaling a Control causes children to overflow, misalign, or clip. Mouse clicks miss the visually scaled area.

**Root cause**: `Control.scale` multiplies rendered pixels after layout. Layout, children, and mouse hit-testing all use the unscaled rect. Unlike Node2D.scale, it does not propagate into the layout system.

**Correct approach**: resize via `size` or `custom_minimum_size`. For zoom effects, change SubViewport resolution.

**Wrong approach**: `Control.scale = Vector2(1.2, 1.2)` for UI animations. Tweening `"scale"` on Control-derived nodes. Using scale to make a button "pop" on hover.

## G3. z_index does not affect input event order [GDScript] [C#]

**Symptom**: a visually-on-top Control (high z_index) does not receive clicks; a behind Control intercepts them.

**Root cause**: input events propagate in reverse scene tree order (deepest child first). z_index only affects rendering, not input dispatch — they are separate systems.

**Correct approach**: for modal input blocking, place a full-screen Control with `mouse_filter = Stop` behind the modal content. Ensure the modal subtree is later in the scene tree than what it occludes.

**Wrong approach**: raising z_index to intercept input. Assuming visual order equals input order. Note: z_index for purely visual ordering (with `mouse_filter = Ignore`) is not a violation — the issue is only when z_index is expected to control input priority.

## G4. No auto-focus on scene load [GDScript] [C#]

**Symptom**: keyboard/gamepad navigation does nothing when a UI scene first appears.

**Root cause**: Godot does not auto-focus any Control on scene entry. The focus graph exists but has no active node.

**Correct approach**: call `grab_focus()` on the initial element in `_ready()` or after show animation.
- C#: `GetNode<Button>("FirstButton").GrabFocus();`

**Wrong approach**: assuming the first focusable Control auto-receives focus.

## G5. Theme resource assignment cuts ancestor inheritance [GDScript] [C#]

**Symptom**: assigning a Theme to a mid-tree node causes its children to lose project-level styling, reverting to engine defaults.

**Root cause**: a node with a Theme resource becomes root of a new theme scope. Cascade lookup stops there — it no longer inherits from ancestors. Missing tokens fall to engine defaults, not parent Theme.

**Correct approach**: use per-property **theme overrides** for selective changes. Only assign a full Theme for completely self-contained visual scopes.

**Wrong approach**: assigning a Theme with only a few custom tokens, expecting unset tokens to inherit from the parent. Creating a "PanelTheme" with only Panel styles and wondering why Buttons inside revert to defaults.

## G6. mouse_filter default silently blocks input to nodes behind [GDScript] [C#]

**Symptom**: clicking a button behind a transparent Panel does nothing.

**Root cause**: many Control types default `mouse_filter` to Stop, consuming mouse events even when fully transparent.

**Correct approach**: set `mouse_filter = Ignore` on overlay/decoration Controls. Use `Pass` if the Control needs to handle input but also let it through.

**Wrong approach**: assuming transparent or empty Controls don't interact with mouse events. Adding a ColorRect overlay for visual effect without setting mouse_filter.

## G7. ScrollContainer swallows directional input from focused children [GDScript] [C#]

**Symptom**: arrow keys on a focused Control inside ScrollContainer scroll instead of navigating focus outward.

**Root cause**: ScrollContainer intercepts unhandled directional input for scrolling before focus navigation can evaluate `focus_neighbor` toward external nodes.

**Correct approach**: set explicit `focus_neighbor_*` on boundary children pointing to external targets. Explicit neighbors take priority over ScrollContainer's scroll interception.

**Wrong approach**: relying on auto-navigation to jump from a scrolled child to a sibling outside the ScrollContainer.

## G8. Hidden Controls steal focus via auto-navigation [GDScript] [C#]

**Symptom**: pressing Tab or D-pad jumps focus to an invisible/off-screen Control.

**Root cause**: `visible = false` does not change `focus_mode`. Godot's spatial auto-navigation considers all Controls with `focus_mode = All`, including hidden ones.

**Correct approach**: when hiding a Control or screen, also set `focus_mode = None` on all focusable children. Restore to `All` when showing.

**Wrong approach**: toggling `visible` alone and expecting focus navigation to skip hidden nodes.

## G9. Control nodes under Node/Node2D lose anchor/layout behavior [GDScript] [C#]

**Symptom**: UI elements don't display, anchor presets have no effect, positions are wrong.

**Root cause**: Control's anchor/layout system only works when the parent is also a Control. Under a Node2D or Node parent, anchor presets are silently ignored. The layout engine never runs for that subtree.

**Correct approach**: ensure the scene root is Control (or a Control subclass) if it contains UI. If mixing gameplay (Node2D) and UI, use separate CanvasLayer branches — one Node2D subtree for gameplay, one Control subtree for UI.

**Wrong approach**: using Node2D as scene root and placing UI Controls as children. Placing Panels or Labels under a Node2D and expecting them to anchor to the viewport.

## G10. Visual nodes under Entity(Node) are invisible [GDScript] [C#]

**Symptom**: ColorRect/Sprite2D exists in the tree but nothing renders. z_index changes have no effect.

**Root cause**: gecs Entity extends Node (not Node2D, not Control). Node is not a CanvasItem, so child CanvasItem nodes (ColorRect, Sprite2D, Control) have no valid canvas parent and are excluded from rendering entirely. z_index cannot fix this because the entire branch is outside the render tree.

**Correct approach**: keep visual nodes in a separate Control or Node2D subtree. Use a render system that maps entity IDs to visual nodes via a Dictionary. The visual nodes live in the UI/render tree; the entity data lives in ECS.

**Wrong approach**: adding ColorRect or Sprite2D as children of an Entity node and expecting them to be visible. Trying to fix with z_index (the problem is not ordering, it's that the branch has no CanvasItem ancestor).

## G11. Control under CanvasLayer needs explicit layout_mode = 1 [GDScript] [C#]

**Symptom**: a TextureRect / Panel / Container placed directly under a CanvasLayer renders at its raw texture / minimum size (e.g. 256×256 for a default TextureRect) and ignores `anchors_preset`, `anchor_*`, and `offset_*` properties.

**Root cause**: when a Control's parent is not a Control (CanvasLayer, Node, Node2D, …), `layout_mode` defaults to 0 (Position mode). Anchor and offset properties are stored on the Control but the layout engine never runs in Position mode, so they have no visible effect. Only `layout_mode = 1` (Anchors mode) activates the anchor/offset pipeline.

**Correct approach**: in the .tscn, set `layout_mode = 1` explicitly on every Control whose parent is a CanvasLayer or any non-Control node. Combine with `anchors_preset = 15` (or whatever preset) + `offset_*` for the full anchor-based stretch.

**Wrong approach**: setting `anchors_preset = 15` alone under a CanvasLayer expecting Full Rect coverage. Setting `anchor_right = 1.0` without `layout_mode = 1`. Wrapping the Control in another empty Control instead of fixing `layout_mode` (the inner Control still defaults to Position mode against its Control parent's empty layout).
