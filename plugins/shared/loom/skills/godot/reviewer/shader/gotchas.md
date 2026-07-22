# Shader Gotchas

Design-level constraints — intentional engine behaviors, not bugs. Stable across Godot 4.x.

## G1. Uniform type mismatch fails silently [GDScript] [C#]

**Symptom**: `set_shader_parameter()` has no visible effect; shader shows default value.

**Root cause**: Godot does not validate types when setting uniforms from script. A type mismatch produces undefined behavior with no error.

**Correct approach**: match types exactly:
- `float` uniform ← `1.0` (GDScript), `1.0f` (C#)
- `vec2/vec3/vec4` ← `Vector2`/`Vector3`/`Color` or `Vector4`
- `sampler2D` ← `Texture2D`

**Wrong approach**:
- `set_shader_parameter("speed", 1)` — int instead of float (GDScript)
- `SetShaderParameter("speed", 1)` — missing `f` suffix (C#)
- `set_shader_parameter("tint", Vector3(...))` for a `vec4` uniform
- Passing `int` for `float` in any arithmetic uniform

## G2. Shared material parameter bleeding [GDScript] [C#]

**Symptom**: changing a shader parameter on one node changes all nodes using the same material.

**Root cause**: Material is a Resource. Multiple nodes referencing the same Resource instance share all state, including uniform values.

**Correct approach**:
- GDScript: `material = material.duplicate()` before per-node params
- C#: `Material = (ShaderMaterial)Material.Duplicate();`
- Or set `resource_local_to_scene = true` on the material resource
- 3D alternative: `instance uniform` for simple scalar/vector overrides

**Wrong approach**:
- `sprite.material.set_shader_parameter(...)` without duplicating first
- Setting per-node values in `_ready()` on a shared material
- Assuming editor "Make Unique" propagates to runtime-spawned instances
- Setting `resource_local_to_scene` on the node instead of the material resource

## G3. instance_uniform is 3D-only [GDScript] [C#]

**Symptom**: `instance uniform` in a `canvas_item` shader compiles but has no per-node effect. Texture-typed instance uniform doesn't work in any shader type.

**Root cause**: `instance uniform` only works on GeometryInstance3D subclasses. CanvasItem nodes have no instance uniform support. Textures are not supported as instance uniform type (max 16, scalar/vector only).

**Correct approach**: for 2D per-node params, use unique materials (G2). For 3D, use `instance uniform` only for scalar/vector types.

**Wrong approach**:
- `shader_type canvas_item;` with `instance uniform float glow;` — compiles, does nothing
- `instance uniform sampler2D tex;` in any shader type — textures not supported
- Using instance uniform to avoid duplicating materials in 2D

## G4. 2D screen texture single-capture [GDScript] [C#]

**Symptom**: second sprite using `hint_screen_texture` shows wrong/stale image; first sprite looks correct.

**Root cause**: in 2D, the engine copies the screen to a back-buffer once — when the first node sampling screen texture is drawn. Subsequent nodes read the same stale snapshot.

**Correct approach**: insert a BackBufferCopy node between each consecutive pair of screen-reading nodes. For N screen-reading nodes, you need N-1 BackBufferCopy nodes.

**Wrong approach**:
- Multiple sprites with `hint_screen_texture` and no BackBufferCopy between them
- Inserting only one BackBufferCopy for 3+ screen-reading nodes
- Assuming each node gets an independent screen capture

## G5. 3D screen texture excludes transparent objects [GDScript] [C#]

**Symptom**: transparent/translucent objects (glass, particles) are invisible in screen-reading shader effects.

**Root cause**: screen texture is captured after the opaque geometry pass but before the transparent pass. Materials using `hint_screen_texture` are themselves classified as transparent.

**Correct approach**: design screen-reading effects knowing only opaque geometry is captured. For transparent objects in the capture, render them to a separate SubViewport.

**Wrong approach**:
- Assuming the screen texture contains the full rendered scene including transparency
- Building refraction/reflection shaders that expect to see translucent objects
- Using screen texture for a "magic lens" effect that should reveal transparent objects
