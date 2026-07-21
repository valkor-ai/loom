---
name: ui-review
description: |
  Reviews Godot UI/Control implementation for known pitfalls.
  Triggers AFTER implementation, when code involves Control, Container, HBoxContainer,
  VBoxContainer, GridContainer, MarginContainer, ScrollContainer, PanelContainer,
  Button, Label, LineEdit, TextEdit, OptionButton, TabContainer, CanvasLayer,
  Theme, StyleBox, focus_mode, mouse_filter, grab_focus, or gui_input.
  Do NOT use this skill for planning or teaching — only for post-implementation review.
---

# UI Review

Post-implementation reviewer for Godot UI code.

## When to trigger

After UI-related code is written or modified. Look for:
- Container nodes (HBox, VBox, Grid, Margin, Panel, Center, Scroll, Tab, Split)
- Control properties (focus_mode, mouse_filter, custom_minimum_size, scale)
- Theme resources or theme overrides
- Focus navigation (grab_focus, focus_neighbor_*)
- CanvasLayer with UI content
- z_index on Control nodes

## Review process

1. Read `gotchas.md`
2. Scan the implemented code against each gotcha
3. For each hit: cite gotcha ID, show offending code, provide fix
4. If no hits, report clean
5. Optionally run `checklist.md` static checks

When you need specific API details, delegate to the **godot-api** skill.
