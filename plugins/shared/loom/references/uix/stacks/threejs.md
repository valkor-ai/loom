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
- Keep DOM overlay controls styled through the same semantic tokens as the rest of the UI.
- Do not let scene code own business form/table/detail state that belongs in UI overlay components.

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
- For generated scenes, inspect both canvas pixels and overlay controls; one without the other is incomplete.

## Scene, Asset, And Overlay Boundary

Keep the render loop, assets, controls, and product UI as separate owners. The
scene provides spatial context; DOM or native overlay components provide labels,
forms, status, and business actions.

```text
scene root -> canvas/camera/controls -> selected object
                                   \\-> overlay context/action/feedback
asset lifecycle -> loading -> ready | fallback | retry
```

- Scene state owns camera, selection, framing, and interaction mode; business records and form drafts stay in the UI/data boundary.
- Overlay panels must preserve readable contrast, pointer/keyboard access, safe placement, and the same semantic tokens as the surrounding product.
- Loading and capability failure need a stable, actionable fallback that keeps the product task understandable without the canvas.
- Asset URLs, preload policy, pixel ratio, disposal, and animation throttling follow the repository's rendering/runtime conventions; do not invent a second asset registry in a component.
- A resize or route change must reconcile camera framing and overlay dimensions without losing selected identity or pending action state.
