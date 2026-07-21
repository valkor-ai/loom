# Navigation Gotchas

Design-level constraints — intentional engine behaviors, not bugs. Stable across Godot 4.x.

## G1. First-frame navigation query returns empty path [GDScript] [C#]

**Symptom**: agent path is empty and agent doesn't move, despite valid target and baked navmesh.

**Root cause**: NavigationServer synchronizes region data after the first physics frame. Any query before that sync returns empty.

**Correct approach**:
```gdscript
# GDScript
func _ready():
    actor_setup.call_deferred()
func actor_setup():
    await get_tree().physics_frame
    navigation_agent.target_position = target
```
```csharp
// C#
public override void _Ready() { CallDeferred(MethodName.ActorSetup); }
private async void ActorSetup() {
    await ToSignal(GetTree(), SceneTree.SignalName.PhysicsFrame);
    _navigationAgent.TargetPosition = _target;
}
```

**Wrong approach**:
- Setting `target_position` directly in `_ready()` / `_Ready()`
- Setting it in `_ready()` with `call_deferred` but without `await physics_frame`
- Setting it in `_enter_tree()` (fires even earlier)

## G2. Avoidance does not reroute paths [GDScript] [C#]

**Symptom**: agent follows original path toward obstacle, gets stuck or oscillates despite `avoidance_enabled = true`.

**Root cause**: RVO avoidance only adjusts frame-by-frame velocity. It never recalculates the underlying path.

**Correct approach**: for static blocking, use `NavigationObstacle` with `affect_navigation_mesh = true` and rebake. For dynamic blocking, detect stuck agents (no progress over N frames) and re-set `target_position`.

**Wrong approach**:
- Relying solely on `avoidance_enabled` to navigate around large or static obstacles
- Assuming `NavigationObstacle` with only `avoidance_enabled` will make agents path around it
- Not implementing any stuck detection when using avoidance in tight spaces

## G3. agent_radius is baked into mesh — no runtime adjustment [GDScript] [C#]

**Symptom**: small agent can't reach visually passable narrow areas, or large agent clips walls at tight corners.

**Root cause**: `agent_radius` shrinks traversable area at bake time. The baked mesh permanently reflects that radius. No runtime per-agent override exists.

**Correct approach**: bake separate meshes per agent size. Use `navigation_layers` to assign agents to size-appropriate meshes.

**Wrong approach**:
- Sharing one navmesh among agents of different sizes
- Expecting the agent's `radius` property to affect pathfinding (it only affects avoidance)
- Trying to set `agent_radius` at runtime expecting path changes

## G4. Off-navmesh target snaps silently [GDScript] [C#]

**Symptom**: agent "arrives" at wrong position. `is_navigation_finished()` returns true while agent is far from intended target.

**Root cause**: if `target_position` is outside the navmesh, pathfinding silently snaps it to the nearest navmesh point. Agent considers itself arrived at the snapped point.

**Correct approach**: after `navigation_finished`, check distance to intended target and handle the unreachable case.

**Wrong approach**:
- Using `is_navigation_finished()` as proof the agent reached the intended destination
- Setting targets from user input / raycasts without navmesh reachability validation
- No post-arrival distance check when targets come from dynamic sources

## G5. Navmesh rebake does not update existing agent paths [GDScript] [C#]

**Symptom**: after rebaking (e.g., wall destroyed), agents continue old paths ignoring newly opened areas.

**Root cause**: agent paths are computed once and cached. Rebaking updates the server graph but does not invalidate existing paths.

**Correct approach**: after rebake, re-set `target_position` on affected agents to trigger fresh path queries.

**Wrong approach**:
- Assuming agents automatically reroute after `bake_navigation_polygon()` or `region_set_navigation_polygon()`
- Rebaking without re-pathing any active agents
- Only rebaking via signal without a follow-up `target_position` re-assignment
