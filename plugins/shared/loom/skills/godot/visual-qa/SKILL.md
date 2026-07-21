---
name: visual-qa
description: |
  Inspect Godot screenshots or short frame sequences for acceptance issues.
  Use after capturing a scene, when checking camera framing, missing assets,
  UI overlap, collision visualization, animation, or a black/empty viewport.
---

# Godot Visual QA

Visual QA is evidence review, not a style contest. Compare the capture with the
task's acceptance criteria and report only issues that affect the requested
behavior, visibility, composition, or operation.

## Capture first

Use the `screenshot` skill or the Godot MCP screenshot tool. Wait for the scene
to render before capturing. For motion, capture a reference frame followed by
several frames at a fixed interval so movement can be checked.

## Review checklist

- The intended scene is running and the viewport is not black or empty.
- The camera shows the requested objects at a usable scale.
- Meshes, textures, lighting, and materials are present rather than placeholders
  when the task requires them.
- Objects that should move or interact visibly change state between frames.
- UI text and controls fit their containers and do not overlap the scene.
- Collision shapes, bodies, and navigation behave consistently with the visual
  geometry when debug overlays are enabled.
- Errors visible in the capture or reported by the runtime are recorded as
  evidence, not guessed away.

## Result format

Record a short result in the Loom task evidence or handoff:

```text
Verdict: pass | warning | fail
Evidence: <capture paths>
Observed: <what the frames show>
Issues: <file and location, or none>
```

Use `fail` for an acceptance-blocking issue, `warning` for a non-blocking
defect, and `pass` only when the supplied criteria are visible in the capture.
