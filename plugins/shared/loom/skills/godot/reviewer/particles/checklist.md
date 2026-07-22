# Particles Checklist

Automated checks to run after implementation. Each check maps to a gotcha.

## Static Checks

### S1. Runtime amount modification → G1
Grep for `.amount =` or `.Amount =` in particle scripts (both CPU and GPU):
- Flag any runtime modification (in `_process`, `_physics_process`, signal handlers, tweens)
- OK: setting `amount` once in `_ready()` or `_init()` (initialization, not runtime)
- OK: `amount_ratio` on GPUParticles, `emitting` toggle on CPUParticles

### S2. Collision node type → G2
If particles are expected to collide:
- Scene must contain `GPUParticlesCollision*` nodes (not physics bodies)
- If sub-emitter uses `at_collision` mode, at least one GPUParticlesCollision node must exist

### S3. Collision AABB overlap → G2
Every GPUParticlesCollision node's transform + extents must overlap the emitter's Visibility AABB:
- Non-overlapping collision nodes have zero effect

### S4. Trail dual requirement → G3
If any GPUParticles node has `trail_enabled = true`:
- `project.godot` → `rendering/renderer/rendering_method` must be `forward_plus` or `mobile`
- Trail draw pass mesh material must have `use_particle_trails = true`
- Same renderer check applies if `turbulence_enabled = true` on the process material

### S5. Sub-emitter amount independence → G4
If a GPUParticles node uses sub-emitters:
- Warn if sub-emitter's `amount` differs from parent's (it will be overridden)

### S6. Zero-acceleration collision trap → G5
If a GPUParticlesCollision node exists, check emitter's material:
- `gravity == Vector3.ZERO` with no acceleration curve → collision will silently fail
- Must have non-zero gravity or acceleration

## Runtime Checks

### R1. Collision verification → G5
Scene: GPUParticles3D with downward gravity + GPUParticlesCollisionBox3D as floor:
```
# After particles settle, all Y positions should be above collision surface
assert(min_particle_y >= collision_surface_y - tolerance, "particles passed through collision")
```

## Compilation

```bash
{godot_path} --headless --quit 2>&1
```
Pass: exit code 0, no errors referencing particle nodes or scripts.
