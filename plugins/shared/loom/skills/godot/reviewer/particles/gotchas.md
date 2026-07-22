# Particles Gotchas

Design-level constraints — intentional engine behaviors, not bugs. Stable across Godot 4.x.

## G1. Changing particle `amount` at runtime kills all particles [GDScript] [C#]

**Symptom**: all visible particles vanish instantly when `amount` property is modified at runtime.

**Root cause**: engine reallocates the entire particle buffer on `amount` change, destroying existing particle state. Applies to both CPUParticles and GPUParticles.

**Correct approach**:
- GDScript: `gpu_particles.amount_ratio = 0.5` (GPU — scales without realloc), `cpu_particles.emitting = false` (CPU — no amount_ratio available)
- C#: `gpuParticles.AmountRatio = 0.5f;`, `cpuParticles.Emitting = false;`

**Wrong approach**:
- `cpu_particles.amount = 50` or `gpu_particles.amount = 50` at runtime
- Tweening `amount` for fade-in/fade-out effects
- Scaling `amount` based on distance for LOD
- Note: setting `amount` once in `_ready()` or `_init()` is fine (initialization before particles exist)

## G2. Particle collision is GPU-only and separate from PhysicsServer [GDScript] [C#]

**Symptom**: particles pass through StaticBody, RigidBody, and Area nodes. Sub-emitter `at_collision` never triggers despite particles visually hitting objects.

**Root cause**: particle collision runs on GPU with dedicated nodes (GPUParticlesCollision*), completely separate from PhysicsServer. 2D has no particle collision at all.

**Correct approach**:
- Add dedicated `GPUParticlesCollisionBox3D` / `GPUParticlesCollisionSphere3D` / `GPUParticlesCollisionHeightField3D` child nodes
- Sub-emitter `at_collision` triggers on GPUParticlesCollision nodes ONLY
- For gameplay hit detection (projectile hits enemy): use PhysicsServer, spawn VFX via script

**Wrong approach**:
- Expecting particles to interact with physics bodies (StaticBody3D, RigidBody3D)
- Using sub-emitter `at_collision` for gameplay hit detection against Area3D or CharacterBody3D
- Trying particle collision in 2D (no collision nodes exist for 2D particles)
- Adding a StaticBody3D as "floor" expecting particles to land on it

## G3. Trails require renderer AND mesh material opt-in [GDScript] [C#]

**Symptom**: `trail_enabled = true` but no visible trail. No error message.

**Root cause**: two independent requirements must both be met: (1) Forward+ or Mobile renderer (not Compatibility), (2) trail draw pass mesh material must set `use_particle_trails = true`. Missing either = silent failure.

**Correct approach**:
- GDScript: `gpu_particles.trail_enabled = true` AND `mesh_material.use_particle_trails = true`
- C#: `gpuParticles.TrailEnabled = true;` AND `meshMaterial.UseParticleTrails = true;`
- Verify `project.godot` → `rendering/renderer/rendering_method` is `forward_plus` or `mobile`

**Wrong approach**:
- Only setting `trail_enabled = true` on the node without touching mesh material
- Enabling trails in a Compatibility renderer project (silent failure)
- Setting `use_particle_trails` on the process material instead of the mesh material
- Enabling turbulence on Compatibility renderer (same renderer constraint)

## G4. Sub-emitter amount is overridden by parent [GDScript] [C#]

**Symptom**: sub-emitter particle count doesn't match what was set on its GPUParticles node.

**Root cause**: the parent emitter's `amount` property overrides the sub-emitter's `amount` at runtime by engine design. No warning emitted.

**Correct approach**: design sub-emitter effects that work within the inherited count. Adjust parent `amount` knowing it affects both.

**Wrong approach**:
- Setting independent `amount` on the sub-emitter GPUParticles node
- Trying to script sub-emitter `amount` at runtime for independent control

## G5. Particles without acceleration silently skip collision [GDScript] [C#]

**Symptom**: particles with initial velocity pass through GPUParticlesCollision nodes despite correct setup.

**Root cause**: collision detection relies on acceleration data for trajectory prediction. Zero acceleration = collision math doesn't trigger.

**Correct approach**:
- GDScript: `material.gravity = Vector3(0, -0.1, 0)` — even minimal acceleration enables collision
- C#: `material.Gravity = new Vector3(0, -0.1f, 0);`
- Also set `gpu_particles.fixed_fps = 60` or higher for fast particles

**Wrong approach**:
- `material.gravity = Vector3.ZERO` with only `initial_velocity` set
- Relying on `linear_accel` curve alone without any gravity
- Debugging collision node placement when the real issue is zero acceleration
