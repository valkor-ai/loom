# Animation Checklist

Automated checks to run after implementation. Each check maps to a gotcha.

## Static Checks

### S0. Expected multi-frame animation missing
If the review brief lists a `grid_sheet` with frame_count > 1, dynamic-mode
evidence, frame sequence, animated actor, or animated FX:
- Flag if the implementation has no AnimationPlayer, AnimationTree,
  AnimatedSprite2D, SpriteFrames, animation state machine, or equivalent
  frame-advance logic.
- Expected: the implementation reads the listed action metadata JSON and wires
  runtime playback from its frame_paths.

### S0b. Static sheet or first-frame collapse
For every multi-frame runtime asset:
- Flag if the sheet final_path is assigned directly to a visible Sprite2D
  texture as the actor/effect.
- Flag if only `frame_paths[0]` or a single extracted frame is used for an
  actor/effect that the brief requires to animate.
- Expected: frames are registered into SpriteFrames, AnimationPlayer tracks, or
  an equivalent playback system.

### S1. Dual-write detection → G1
Grep for `AnimationPlayer.play(` or `.play(` in scripts that also reference `AnimationTree`:
- Flag if both `AnimationTree.active = true` and `AnimationPlayer.play()` exist in the same scene/script
- Expected: only one control path per property

### S2. Resource mutation without duplicate → G2
Grep for runtime resource property writes: `set_animation_speed`, `set_animation_loop`, `.length =`, `.step =` on Animation/SpriteFrames:
- Flag if no `.duplicate()` call precedes the mutation
- Expected: `resource = resource.duplicate()` before any modification

### S3. Process mode for physics characters → G3
For every AnimationTree attached to a CharacterBody2D or RigidBody2D:
- `callback_mode_process` must be `ANIMATION_PROCESS_PHYSICS`
- Flag if set to IDLE or left at default

## Runtime Checks

### R1. Pool lifecycle reset → G4
Simulate pool return and retrieve:
```
playback.travel("die")
await get_tree().create_timer(1.0).timeout
tree.active = false
# pool retrieve
playback.start("idle")
tree.active = true
await get_tree().process_frame
assert(playback.get_current_node() == "idle", "pool state not reset")
```

### S4. queue_free in animation/render code → G5
Grep scripts for `queue_free()` in systems or functions that also create replacement nodes:
- Flag if `queue_free()` and node creation happen in the same frame
- Expected: use `free()` for immediate removal, or `visible = false` before replacement

### S5. Tween creation without state guard → G6
Grep scripts for `create_tween()` inside `process()` or `_process()`:
- Flag if no state check prevents re-entry (e.g., missing `if state == "idle"` guard)
- Expected: state transitions to non-idle immediately after Tween creation

### S6. Temporary animated FX lifecycle
For projectile, impact, pickup, slash, aura, or feedback effects:
- Flag if the implementation spawns or shows the animated effect without a
  corresponding animation_finished handler, timer, tween completion,
  queue_free/free, or state clear.
- Expected: temporary FX disappear or clear after playback while persistent
  actors remain alive.

## Compilation

```bash
{godot_path} --headless --quit 2>&1
```
Pass: exit code 0, no errors referencing AnimationPlayer, AnimationTree, or SpriteFrames.
