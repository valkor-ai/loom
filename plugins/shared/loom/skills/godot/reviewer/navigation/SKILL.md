---
name: navigation-review
description: |
  Reviews Godot navigation implementation for known pitfalls.
  Triggers AFTER implementation, when code involves NavigationAgent2D,
  NavigationAgent3D, NavigationRegion2D, NavigationRegion3D, NavigationLink2D,
  NavigationLink3D, NavigationObstacle2D, NavigationObstacle3D, target_position,
  TargetPosition, avoidance_enabled, bake_navigation_polygon, navigation_layers,
  velocity_computed, safe_velocity, is_navigation_finished.
  Do NOT use this skill for planning or teaching — only for post-implementation review.
---

# Navigation Review

Post-implementation reviewer for Godot navigation code. Checks against known gotchas that LLMs consistently get wrong.

## When to trigger

After navigation-related code is written or modified. Look for:
- Navigation nodes (NavigationAgent2D/3D, NavigationRegion2D/3D, NavigationLink2D/3D, NavigationObstacle2D/3D)
- `target_position` / `TargetPosition` assignment
- `avoidance_enabled`, `velocity_computed`, `safe_velocity`
- `bake_navigation_polygon()`, `navigation_layers`
- `is_navigation_finished()`, `get_next_path_position()`

## Review process

1. Read `gotchas.md`
2. Scan the implemented code against each gotcha
3. For each hit: cite gotcha ID, show offending code, provide fix
4. If no hits, report clean
5. Optionally run `checklist.md` static checks for automated verification

When you need specific API details, delegate to the **godot-api** skill.
