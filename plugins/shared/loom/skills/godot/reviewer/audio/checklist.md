# Audio Checklist

Automated checks to run after implementation. Each check maps to a gotcha.

## Static Checks

### S1. Compressed stream preload budget → G1
Grep for `preload("` with `.ogg"` or `.mp3"`:
- Flag any compressed stream > 30 seconds that is `preload()`ed
- Multiple compressed preloads → estimate total PCM RAM and flag if excessive

### S2. Polyphony separation → G2
For every AudioStreamPlayer used for SFX:
- Important one-shot sounds (explosion, hit, death) use separate Player nodes from rapid-fire SFX (footsteps, coins)
- A single Player handling both critical and non-critical sounds is a violation

### S3. Bus modification thread safety → G3
Grep for `AudioServer.add_bus`, `remove_bus`, `add_bus_effect`, `remove_bus_effect`, `move_bus`, `swap_bus_effects`:
- Each call must be wrapped in `AudioServer.lock()` / `unlock()`
- OR confirmed as init-only (called in `_ready()` before audio plays)
- Flag any runtime structural change not inside a lock block

### S4. Composite stream finished signal → G4
Grep for `.finished.connect` and `await .finished`:
- If the Player's stream is Synchronized, Playlist (looping), or any looping stream → flag
- These must use explicit `stop()` + state tracking, not `finished`

## Runtime Checks

### R1. Polyphony stress test → G2
Trigger rapid-fire SFX beyond `max_polyphony` limit while important sound plays on dedicated Player:
```
assert(important_player.playing == true, "important sound was cut")
```

### R2. Composite stream finished test → G4
Play a looping AudioStream, await `finished` with timeout:
```
var timed_out = false
var timer = get_tree().create_timer(3.0)
timer.timeout.connect(func(): timed_out = true; player.stop())
await player.finished
assert(timed_out, "finished should not fire on looping stream — cleanup must use explicit stop()")
```

## Compilation

```bash
{godot_path} --headless --quit 2>&1
```
Pass: exit code 0, no errors referencing audio nodes or scripts.
