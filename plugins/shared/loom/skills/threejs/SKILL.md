---
name: threejs
description: |
  Build, debug, and verify Three.js browser games through Loom. Use only when
  the request mentions Three.js, React Three Fiber, WebGL game work, or the
  active workspace declares a three, @react-three/fiber, or @react-three/drei
  dependency. Do not load for unrelated web projects.
---

# Three.js Skills for Loom

Loom owns delivery state, task planning, repair, and handoff. Start non-trivial
work through `@loom plan`; treat the specialist skills below as technical and
verification guidance, not as an alternative workflow.

## Choose the smallest skill

| Work | Skill |
| --- | --- |
| Game design, core loop, input, camera, physics, or game feel | `threejs-gameplay-systems` |
| Models, materials, shaders, lighting, or render quality | `threejs-aaa-graphics-builder` |
| HUDs, menus, overlays, touch controls, or responsive game UI | `threejs-game-ui-designer` |
| Blank canvas, runtime errors, asset loading, input, or performance | `threejs-debug-profiler` |
| Playtesting, screenshots, canvas diagnostics, or release checks | `threejs-qa-release` |
| Explicit 3D, image, or audio asset generation | `threejs-3d-generator`, `threejs-image-generator`, or `threejs-audio-generator` |

Do not load every specialist for a small fix. Browser games with visual or
interactive acceptance criteria should finish with the relevant browser
evidence recorded in the Loom task result.

## Delivery loop

For a new game or a substantial change, begin in the project root:

```text
@loom plan Build a Three.js game with a playable movement loop and verified browser controls
@loom continue
```

Run the project's build and focused behavior checks first. Use canvas
inspection, screenshots, or playtests only when they are relevant to the task.
When a specialist skill recommends a workflow that conflicts with a Loom MCP
request, follow the MCP request.

## Third-party material

The specialist skill directories were imported from an MIT-licensed upstream
skill pack. Its license is retained in `UPSTREAM_LICENSE`.
