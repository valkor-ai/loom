# Physics Gotchas

Design-level constraints — intentional engine behaviors, not bugs. Stable across Godot 4.x.

## G1. Physics callbacks cannot modify physics state [GDScript] [C#]

**Symptom**: "Can't change state while flushing queries" inside `body_entered`, `body_exited`, `area_entered`, `area_exited`, or `_integrate_forces()`.

**Root cause**: Godot locks the physics space during collision callbacks. Operations that mutate the broadphase invalidate the iterator.

**Correct approach**: defer all state-modifying operations:
- GDScript: `call_deferred("queue_free")`, `shape.set_deferred("disabled", true)`
- C#: `CallDeferred(Node.MethodName.QueueFree)`, `shape.SetDeferred(CollisionShape2D.PropertyName.Disabled, true);`

**Wrong approach**: `queue_free()`, `add_child()`, `remove_child()`, or `shape.disabled = true` directly inside physics callbacks.

## G2. Frame-rate dependent drag [GDScript] [C#]

**Symptom**: damping/friction feels different at different frame rates.

**Root cause**: `speed *= (1 - drag)` is exponential decay per tick, not per second. At 60 Hz vs 120 Hz the remaining speed diverges dramatically.

**Correct approach**: `speed *= exp(-rate * delta)`

**Wrong approach**:
- `speed *= (1.0 - drag)` in `_physics_process()` or `_process()`
- `velocity *= drag_factor` per tick without `delta` — same exponential-per-tick error
- Using `lerp(speed, 0, drag)` per tick thinking lerp is frame-rate safe (it isn't without delta-based weight)
- Multiplying by `(1 - drag * delta)` — linear approximation that goes negative at large delta or high drag

## G3. Default collision mask misses non-default layers [GDScript] [C#]

**Symptom**: newly created physics body falls through floor or ignores walls silently. No error, no warning.

**Root cause**: new bodies default to `collision_layer = 1, collision_mask = 1`. If terrain uses layer 3, the default mask doesn't include it.

**Correct approach**: every CollisionObject2D (body or Area) must have **both** `collision_layer` and `collision_mask` explicitly set **in code** (in `_ready()` or via `@export`). "Set it in the editor" is not sufficient — editor settings are invisible to code review and silently reset if the scene is re-created.

**Wrong approach**:
- Only setting layer without mask (or vice versa)
- Assuming default mask collides with everything
- Deferring layer/mask setup to "configure in the Inspector" without code-level assignment
- Setting layers for some bodies (e.g. bullets) but leaving others (e.g. player, enemies) at defaults

## G4. Bitmask value vs editor layer number [GDScript] [C#]

**Symptom**: `collision_layer = 4` puts the object on editor layer 3, not layer 4.

**Root cause**: `collision_layer`/`collision_mask` are bitmasks. Layer 1 = 1, Layer 2 = 2, Layer 3 = 4.

**Correct approach**: use `set_collision_layer_value(layer_number, true)` which takes 1-based number. Or compute: `1 << (layer_number - 1)`.

**Wrong approach**: `collision_layer = N` thinking N is the layer number.

## G5. Collision shapes slightly smaller than tile grid [GDScript] [C#]

**Symptom**: character snags on 1-tile corridor entrances or gets stuck on tile corners.

**Root cause**: exact tile-sized shapes create floating-point overlaps at tile boundaries.

**Correct approach**: make collision shapes 2-4 px smaller than the tile. Visual sprite remains tile-sized.

**Wrong approach**:
- Collision shape matching tile grid exactly (e.g., 16×16 shape for 16×16 tiles)
- Shrinking only one axis (e.g., width but not height) — still snags on horizontal edges
- Using a capsule shape at exact tile width — rounded ends don't eliminate corner catches at tile seams
- Adjusting movement code to compensate instead of fixing the shape (mask the symptom)

## G6. Objects spawned inside active Area trigger immediate callbacks [GDScript] [C#]

**Symptom**: pickup spawned inside an Area2D immediately fires `area_entered` and gets destroyed same frame.

**Root cause**: the physics engine detects overlap on the next physics step. Objects placed inside a monitoring Area trigger enter signals immediately.

**Correct approach** (choose by context):
- Immunity window — track alive time, ignore callbacks briefly
- Score on `_exited` instead of `_entered`
- Disable Area monitoring until triggering effect finishes
- Spawn outside the Area

**Wrong approach**:
- Assuming newly spawned objects won't trigger existing Area callbacks
- Destroying the object in `area_entered` without checking alive time (kills it frame 0)
- Using `monitoring = false` at spawn without re-enabling it (permanently disables the Area)
- Checking `is_inside_tree()` as a spawn guard — true immediately after `add_child()`, doesn't help
