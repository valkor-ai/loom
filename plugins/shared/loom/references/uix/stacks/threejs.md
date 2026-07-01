# Three.js And Canvas UIX Stack

Use for Three.js, WebGL, canvas, game-like, simulation, or scene-heavy web UI.

## Rules

- Verify the primary canvas is nonblank, correctly sized, and framed.
- Keep UI controls outside the scene path unless they intentionally overlay the scene.
- Handle asset loading, missing assets, resize, device pixel ratio, input modality, and reduced-motion fallbacks.
- Use bounded animation loops and dispose resources when views unmount.
- Record screenshot, canvas-pixel, or browser evidence when possible.
