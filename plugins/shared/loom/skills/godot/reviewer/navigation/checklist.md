# Navigation Checklist

Automated checks to run after implementation. Each check maps to a gotcha.

## Static Checks

### S1. First-frame sync pattern → G1
Grep scripts that use NavigationAgent:
- GDScript: `target_position` must NOT be set in `_ready()` without a preceding `await get_tree().physics_frame`
- C#: `TargetPosition` must NOT be set in `_Ready()` without a preceding `await ToSignal(GetTree(), SceneTree.SignalName.PhysicsFrame)`
- Acceptable: set in a deferred function that awaits a physics frame first

### S2. Navigation layer assignment → G3
For agents with custom `navigation_layers`:
- At least one NavigationRegion exists with a matching layer
- Agents of different sizes use different layers with separate baked meshes

### S3. Off-navmesh target handling → G4
Grep scripts that set `target_position` / `TargetPosition`:
- If target comes from user input, raycasts, or dynamic sources: verify there is a post-arrival distance check or navmesh reachability validation
- Hardcoded targets on known navmesh areas are acceptable

### S4. Avoidance stuck handling → G2
Grep scripts that enable `avoidance_enabled`:
- Must have stuck detection logic (e.g., progress check over N frames, or distance delta threshold)
- If NavigationObstacle is used for static blocking: must also have `affect_navigation_mesh = true` and rebake call
- Relying solely on avoidance to navigate around large or static obstacles is a G2 violation

### S5. Rebake-then-repath pattern → G5
Grep for `bake_navigation_polygon()` or `region_set_navigation_polygon()`:
- After the bake call (same function or via signal), affected agents must have `target_position` re-set
- Bake without re-pathing active agents is a G5 violation

## Runtime Checks

### R1. Path validity test → G1
```gdscript
await get_tree().physics_frame
agent.target_position = target_pos
await get_tree().physics_frame
assert(agent.get_current_navigation_path().size() > 0, "path empty after sync")
```

### R2. Arrival accuracy test → G4
```gdscript
# After agent movement completes:
assert(agent.is_navigation_finished(), "agent never reached target")
assert(agent.global_position.distance_to(intended_target) < threshold,
    "arrived at snapped position, not intended target")
```

### R3. Rebake path update test → G5
```gdscript
# Agent has path through region X → modify geometry → rebake
agent.target_position = agent.target_position  # force re-query
await get_tree().physics_frame
assert(new_path != old_path, "path did not update after rebake")
```

### R4. Multi-size agent navmesh test → G3
Two agents with different radii, each assigned to a separate navigation layer with appropriately baked meshes:
```gdscript
# Narrow corridor passable by small agent only
await get_tree().physics_frame
small_agent.target_position = narrow_target
large_agent.target_position = narrow_target
await get_tree().physics_frame
assert(small_agent.get_current_navigation_path().size() > 0, "small agent should path through narrow corridor")
assert(large_agent.get_current_navigation_path().size() == 0 or
    large_agent.global_position.distance_to(narrow_target) > threshold,
    "large agent should not reach narrow corridor target")
```

## Compilation

```bash
{godot_path} --headless --quit 2>&1
```
Pass: exit code 0, no errors referencing navigation nodes or scripts.
