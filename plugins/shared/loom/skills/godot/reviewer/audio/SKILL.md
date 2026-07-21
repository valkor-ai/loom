---
name: audio-review
description: |
  Reviews Godot Audio implementation for known pitfalls.
  Triggers AFTER implementation, when code involves AudioStreamPlayer,
  AudioStreamPlayer2D, AudioStreamPlayer3D, AudioServer, AudioBus,
  AudioEffect, AudioStream, or .finished signal on audio players.
  Do NOT use this skill for planning or teaching — only for post-implementation review.
---

# Audio Review

Post-implementation reviewer for Godot audio code. Checks against known gotchas that LLMs consistently get wrong.

## When to trigger

After audio-related code is written or modified. Look for:
- Audio player nodes (AudioStreamPlayer, AudioStreamPlayer2D, AudioStreamPlayer3D)
- AudioServer bus manipulation (`add_bus`, `remove_bus`, `add_bus_effect`, `remove_bus_effect`)
- `preload()` of `.ogg` or `.mp3` files
- `finished` signal connections or `await .finished` on audio players
- Polyphony configuration or SFX pooling patterns

## Review process

1. Read `gotchas.md`
2. Scan the implemented code against each gotcha
3. For each hit:
   - Cite the gotcha ID (e.g. G1)
   - Show the offending code
   - Provide the fix
4. If no hits, report clean
5. Optionally run `checklist.md` static checks for automated verification

When you need specific API details, delegate to the **godot-api** skill.
