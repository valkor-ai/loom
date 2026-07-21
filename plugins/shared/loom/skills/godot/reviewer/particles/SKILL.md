---
name: particles-review
description: |
  Reviews Godot Particles implementation for known pitfalls.
  Triggers AFTER implementation, when code involves GPUParticles2D, GPUParticles3D,
  CPUParticles2D, CPUParticles3D, ParticleProcessMaterial, trail_enabled,
  sub-emitters, GPUParticlesCollision, or particle amount/emission control.
  Do NOT use this skill for planning or teaching — only for post-implementation review.
---

# Particles Review

Post-implementation reviewer for Godot particle code. Checks against known gotchas that LLMs consistently get wrong.

## When to trigger

After particle-related code is written or modified. Look for:
- Particle nodes (GPUParticles2D, GPUParticles3D, CPUParticles2D, CPUParticles3D)
- ParticleProcessMaterial or `shader_type particles`
- Trail configuration (`trail_enabled`, `use_particle_trails`)
- Sub-emitter setup (`sub_emitter_mode`, `sub_emitter_*`)
- Collision nodes (GPUParticlesCollisionBox3D, GPUParticlesCollisionSphere3D, etc.)
- Runtime particle property changes (`.amount`, `.emitting`)

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
