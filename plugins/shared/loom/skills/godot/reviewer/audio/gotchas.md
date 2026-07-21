# Audio Gotchas

Design-level constraints — intentional engine behaviors, not bugs. Stable across Godot 4.x.

## G1. Compressed stream RAM is ~10x disk size [GDScript] [C#]

**Symptom**: memory budget exceeded despite using small OGG/MP3 files.

**Root cause**: compressed audio is decoded into PCM buffers at runtime. A 5 MB OGG → ~50 MB RAM. This is inherent to streaming decompression, not a bug.

**Correct approach**: budget by PCM size: `sample_rate × channels × 2 bytes × duration`. Use WAV for short SFX (already uncompressed, no surprise). Only compress long audio, and load on demand rather than `preload()`.

**Wrong approach**:
- Assuming a 2 MB OGG uses ~2 MB RAM
- `preload()`ing many compressed tracks (10 OGGs × 50 MB = 500 MB)
- Budgeting disk space instead of decoded PCM size in design docs

## G2. max_polyphony cuts oldest sound, no priority [GDScript] [C#]

**Symptom**: important one-shot sound (explosion, critical hit) silently dropped during dense audio.

**Root cause**: when `max_polyphony` is exceeded, the engine cuts the oldest instance (FIFO). There is no priority system — a quiet ambient loop and a critical explosion are treated equally.

**Correct approach**: give important sounds their own dedicated Player node, separate from rapid-fire SFX. Optionally duck other buses to ensure audibility.

**Wrong approach**:
- Single shared Player for all SFX (footsteps + explosions compete for slots)
- Raising `max_polyphony` on one Player and assuming all sounds play
- Assuming the engine has any sound priority or importance system

## G3. AudioServer.lock() required for runtime bus changes [GDScript] [C#]

**Symptom**: intermittent audio glitches, crashes, or silent output when adding/removing buses or effects at runtime.

**Root cause**: the audio thread runs independently. Modifying bus structure without locking creates a race condition with the mix thread.

**Correct approach**: wrap structural changes in `AudioServer.lock()` / `unlock()`. Better: pre-define all buses in `default_bus_layout.tres` and avoid runtime structural changes entirely.

**Wrong approach**:
- `AudioServer.add_bus_effect(idx, effect)` without `lock()`/`unlock()`
- `AudioServer.remove_bus(idx)` without locking
- Any of: `add_bus`, `remove_bus`, `add_bus_effect`, `remove_bus_effect`, `move_bus`, `swap_bus_effects` called outside a lock block
- Assuming bus parameter changes (volume, mute) also need locking (they don't — only structural changes do)

## G4. finished signal unreliable with composite/looping streams [GDScript] [C#]

**Symptom**: `finished` signal never fires, `await player.finished` hangs forever, or cleanup code never executes.

**Root cause**: composite streams (AudioStreamSynchronized, looping Playlist) manage their own lifecycle. Synchronized never emits `finished` by design. Looping streams never "finish".

**Correct approach**: use explicit `stop()` + state tracking for lifecycle management. Do not rely on `finished` for composite or looping streams.

**Wrong approach**:
- `await player.finished` when stream is Synchronized (hangs forever)
- `player.finished.connect(cleanup)` on a looping stream (never called)
- Using `finished` to sequence tracks when any track might loop
- Assuming all AudioStreamPlayer types emit `finished` uniformly
