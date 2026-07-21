# Shader Checklist

Automated checks to run after implementation. Each check maps to a gotcha.

## Static Checks

### S1. Uniform type match → G1
For every `set_shader_parameter("name", value)` call in scripts:
- Resolve which `.gdshader` the material references
- Check the `uniform` declaration type matches the passed value type
- Flag: `int` literal for `float` uniform, wrong vector dimension, non-Texture2D for `sampler2D`

### S2. Shared material mutation → G2
For every `set_shader_parameter()` call in scripts:
- Check target material is either `.duplicate()`'d, `resource_local_to_scene = true`, or assigned to a single node
- Flag: `node.material.set_shader_parameter(...)` without prior duplication on a material used by multiple nodes

### S3. instance_uniform scope → G3
Scan all `.gdshader` files:
- If `shader_type canvas_item` and `instance uniform` both appear → flag (3D-only)
- If `instance uniform sampler2D` appears in any shader type → flag (textures not supported)

### S4. 3D screen texture transparency → G5
If a `spatial` shader uses `hint_screen_texture` and the scene contains transparent meshes:
- Flag that transparent objects will not appear in the capture
- Verify design explicitly accounts for opaque-only limitation

### S5. 2D screen texture capture order → G4
Grep `.gdshader` files with `shader_type canvas_item` for `hint_screen_texture`:
- If 2+ nodes in the same scene use shaders with `hint_screen_texture`: flag
- Must have BackBufferCopy node between each consecutive pair of screen-reading nodes
- N screen-reading nodes require N-1 BackBufferCopy nodes

## Runtime Checks

### R1. Material independence test → G2
Two nodes sharing the same shader with `resource_local_to_scene = true`:
- Set different parameter values on each
- Assert visual difference between the two nodes

### R2. Screen-reading order test → G4
Two overlapping sprites both using `hint_screen_texture`:
- Without BackBufferCopy → assert second sprite shows stale image
- With BackBufferCopy between them → assert both show correct result

## Compilation

```bash
{godot_path} --headless --quit 2>&1
```
Pass: exit code 0, no `SHADER ERROR` or `Parse Error` in output.
