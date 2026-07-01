# UIX Stack: Three.js

Use for Three.js, React Three Fiber, WebGL, canvas-heavy 3D, configurators, games, simulations, and immersive scenes.

## Structure

- Keep scene setup, asset loading, controls, UI overlay, and business state separated.
- Use the repo's existing Three.js/R3F conventions when present.
- The scene should be the primary surface for immersive tasks, not a decorative preview inside a card.

## Scene Module Split

```text
scene/
  create-scene
  camera
  lighting
  controls
  assets
  interactions
ui/
  SceneHud
  SceneInspector
  LoadingOverlay
```

## Implementation Rules

- Define stable canvas dimensions and resize behavior.
- Provide loading and fallback states for assets, WebGL, and shader failures.
- Keep camera, lighting, controls, and object framing intentional.
- Overlay UI must remain readable and must not cover the subject.
- Dispose resources and avoid runaway animation loops.
- Respect reduced motion or provide lower-motion controls when practical.

## Render Pattern

```css
.scene-root { position: relative; width: 100vw; height: 100dvh; overflow: hidden; }
.scene-canvas { position: absolute; inset: 0; }
.scene-overlay { position: absolute; inset: 0; pointer-events: none; }
.scene-overlay > * { pointer-events: auto; }
```

## Verification

- Check nonblank canvas pixels.
- Check desktop and mobile framing.
- Confirm assets load and controls respond.
- Verify the app remains interactive after resizing or route changes.
