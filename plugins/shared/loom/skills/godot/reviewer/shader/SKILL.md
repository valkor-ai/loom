---
name: shader-review
description: |
  Reviews Godot Shader/Material implementation for known pitfalls.
  Triggers AFTER implementation, when code involves ShaderMaterial,
  set_shader_parameter, .gdshader, VisualShader, hint_screen_texture,
  BackBufferCopy, instance uniform, or uniform declarations.
  Do NOT use this skill for planning or teaching — only for post-implementation review.
---

# Shader Review

Post-implementation reviewer for Godot shader code. Checks against known gotchas that LLMs consistently get wrong.

## When to trigger

After shader-related code is written or modified. Look for:
- ShaderMaterial, `.gdshader` files, VisualShader resources
- `set_shader_parameter()` / `SetShaderParameter()` calls
- `hint_screen_texture`, BackBufferCopy nodes
- `instance uniform` declarations
- Uniform type assignments in scripts

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
