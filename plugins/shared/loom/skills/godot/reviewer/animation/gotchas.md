# Animation Gotchas

Design-level constraints — intentional engine behaviors, not bugs. Stable across Godot 4.x.

## G1. AnimationPlayer and AnimationTree dual-write conflict [GDScript] [C#]

**Symptom**: code-set properties revert immediately; `AnimationPlayer.play()` has no visible effect.

**Root cause**: AnimationTree evaluates every frame and overwrites all tracked properties unconditionally — including values set by code and by direct AnimationPlayer playback.

**Correct approach**: ONE control path per property.
- Simple playback → `AnimationPlayer.play()` with `AnimationTree.active = false`
- Complex blending → all control through AnimationTree parameters
- Need code override on a specific property → use AnimationTree's track filter to exclude it

**Wrong approach**:
- Calling `AnimationPlayer.play()` while `AnimationTree.active = true` on overlapping properties
- Setting a property in `_process()` / `_physics_process()` that an active AnimationTree also tracks (value reverts next frame)
- Toggling `AnimationTree.active` on/off per-frame to "let code through" (causes blend state reset)

## G2. Animation resource shared mutation [GDScript] [C#]

**Symptom**: changing animation speed, frames, or other properties on one instance affects ALL instances using the same resource.

**Root cause**: SpriteFrames, Animation, and AnimationLibrary are Resources — shared by default. Multiple nodes from the same PackedScene or `.tres` reference one object in memory.

**Correct approach**:
- GDScript: `resource = resource.duplicate()` before any runtime modification
- C#: `Resource = (ResourceType)Resource.Duplicate();`
- For per-instance speed only: use `AnimationPlayer.speed_scale` or `AnimatedSprite2D.speed_scale` (instance properties, safe without duplication)

**Wrong approach**:
- `sprite_frames.set_animation_speed("walk", 20)` without duplicating first — changes all instances
- `animation_player.get_animation("walk").length = 2.0` — mutates the shared Animation resource
- Assuming each instantiated scene gets its own copy of sub-resources (it doesn't unless "Local to Scene" is checked)

## G3. Process mode mismatch causes one-frame desync [GDScript] [C#]

**Symptom**: animated properties appear one frame late — visible as jitter, sliding, or foot skating on physics-driven characters.

**Root cause**: AnimationTree defaults to `ANIMATION_PROCESS_IDLE`. If game logic runs in `_physics_process`, animation output lags one frame behind physics state.

**Correct approach**: match `callback_mode_process` to the consumer:
- Physics-driven movement → `ANIMATION_PROCESS_PHYSICS`
- Visual-only (UI, particles) → `ANIMATION_PROCESS_IDLE`

**Wrong approach**:
- Leaving default IDLE mode for animations driving physics-relevant properties (position, velocity blend)
- Setting PHYSICS mode for purely cosmetic animations (wastes physics budget)

## G4. StateMachine retains state across deactivation [GDScript] [C#]

**Symptom**: pooled entity plays its death animation immediately after respawn, or resumes mid-combo instead of idle.

**Root cause**: `AnimationTree.active = false` pauses evaluation but does NOT reset the state machine's current node. On reactivation, it resumes from the last state.

**Correct approach**: on pool retrieve, force-reset before reactivation:
```
playback.start("idle")   # force jump to idle, ignore transitions
tree.active = true
```

**Wrong approach**:
- Setting `tree.active = true` without resetting state — resumes from "die" or last combat state
- Using `playback.travel("idle")` instead of `start()` — travel follows transition rules, may not reach idle if no path exists from current state
- Resetting state AFTER setting active (tree evaluates one frame of stale state)

## G5. queue_free() delays one frame — causes visual overlap [GDScript] [C#]

**Symptom**: old and new visual nodes overlap for one frame, causing flicker or ghosting during animations.

**Root cause**: `queue_free()` marks a node for deletion at the end of the current frame, not immediately. If new nodes are created in the same frame, both old and new coexist for one render pass.

**Correct approach**: use `free()` for immediate removal when visual consistency matters, or hide the old node (`visible = false`) before creating the replacement.

**Wrong approach**: using `queue_free()` in animation/render code and assuming the node is gone immediately. Using a "destroy old layer + create new layer" pattern for animation transitions.

## G6. Tween/animation re-created every frame without state guard [GDScript] [C#]

**Symptom**: first input works, subsequent inputs are ignored. Animation count grows infinitely.

**Root cause**: a state-machine-driven animation system stays in "animating" state and creates a new Tween every `process()` call. The completion counter never reaches zero because new tweens keep being added.

**Correct approach**: transition to a "tweening" or "in_progress" state immediately after creating the Tween. Only create new Tweens when in "idle" state.
```gdscript
# Correct state machine
if state == "animating":
    create_tween()
    state = "tweening"     # prevent re-entry
```

**Wrong approach**: creating tweens while `state == "animating"` without changing state, allowing `process()` to create another tween next frame.
