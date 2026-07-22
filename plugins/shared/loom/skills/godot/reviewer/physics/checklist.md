# Physics Checklist

Automated checks to run after implementation. Each check maps to a gotcha.

## Static Checks

### S1. Physics callback safety → G1
Grep scripts connected to `body_entered`, `body_exited`, `area_entered`, `area_exited`:
- No direct `queue_free()`, `add_child()`, `remove_child()`
- No direct `shape.disabled = ...`
- All such ops use `call_deferred()` or `set_deferred()`

### S2. Drag pattern → G2
Grep for `*= (1` or `*= 1 -` in physics scripts:
- Flag `speed *= (1 - drag)` without delta compensation
- Expected: `exp(-rate * delta)` pattern

### S3. Collision layer explicit assignment → G3
For every CollisionObject2D in the project:
- Has at least one CollisionShape2D child with a non-null shape
- `collision_layer` is not 0
- `collision_mask` is explicitly set (not left at default)

### S4. Bitmask assignment → G4
Grep for raw integer assignment: `collision_layer\s*=\s*\d+` or `collision_mask\s*=\s*\d+`
- Flag and recommend `set_collision_layer_value()` / `set_collision_mask_value()`

### S5. Collision shape vs tile size → G5
For every CollisionShape2D on a CharacterBody2D or RigidBody2D navigating tiled environments:
- Compare shape extents to tile_size
- Flag shapes whose dimensions match tile_size exactly (should be 2-4 px smaller)

## Runtime Checks

### R1. Penetration test → G3
Drop a RigidBody2D onto a StaticBody2D floor:
```
assert(body.global_position.y <= floor_y + tolerance, "penetrated floor")
```

### R2. Layer interaction test → G4
Two bodies on different layers:
- Matching layer/mask → assert collision detected
- Non-matching → assert no collision

### R3. Spawn-inside-Area test → G6
Spawn an object inside a monitoring Area2D, advance one physics frame:
```
assert(is_instance_valid(spawned_object), "object destroyed by immediate Area callback")
```

## Compilation

```bash
{godot_path} --headless --quit 2>&1
```
Pass: exit code 0, no errors referencing physics nodes or scripts.
