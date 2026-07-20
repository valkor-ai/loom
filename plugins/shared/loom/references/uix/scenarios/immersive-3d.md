# UIX Scenario: Immersive 3D

Use for Three.js/WebGL/canvas scenes, product configurators, spatial tools, games, simulations, and immersive interactive visuals.

## Baseline

- The primary scene is full-bleed or dominant, not trapped inside a decorative card.
- The scene must render nonblank, be correctly framed, and respond to expected interaction.
- Density is `immersive`.
- Controls support the scene task without covering critical visual content.

## Scene Layout

```html
<main data-region="scene-page">
  <canvas data-region="scene-canvas"></canvas>
  <section data-region="hud">
    <header data-region="hud-top"></header>
    <aside data-region="scene-inspector"></aside>
    <footer data-region="scene-controls"></footer>
  </section>
</main>
```

```css
.scene-page {
  position: relative;
  width: 100vw;
  height: 100dvh;
  overflow: hidden;
  background: var(--surface);
}

.scene-canvas {
  position: absolute;
  inset: 0;
}

.scene-hud {
  position: absolute;
  inset: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  padding: var(--space-6);
  pointer-events: none;
}

.scene-hud > * {
  pointer-events: auto;
}
```

## Required Patterns

- Stable canvas sizing and resize handling.
- Loading and fallback state for assets/shaders/WebGL support.
- Scene controls: camera, zoom, rotate, reset, selection, mode toggle, or inspector as required.
- Overlay UI with readable contrast and safe placement.
- Reduced-motion or performance fallback when practical.

## Control Composition

- Keep primary controls docked or overlaid in predictable zones; avoid covering the object/scene center.
- Inspector panels should be collapsible or responsive when they compete with the scene.
- Use labels/tooltips for unfamiliar scene controls.
- If the scene represents a product/configuration/game state, visible UI must expose the current mode, selected object, and available action.

## Verification

- Verify canvas pixels are nonblank.
- Check desktop and mobile framing.
- Confirm referenced assets load.
- Confirm animation/interaction continues after initial render.
- Check controls do not occlude the subject.
- Check resize handling and device pixel ratio behavior when practical.

## Avoid

- Static placeholder canvas.
- Dark blurred background with no inspectable object.
- Controls that occlude the subject or cannot be used by touch.

## Loading, Fallback, And Performance

An immersive surface is incomplete until it behaves predictably before assets
load, when WebGL is unavailable, and after the viewport changes.

```text
initial loading -> asset progress or skeleton -> interactive scene
                                      \\-> fallback explanation + usable action
```

- Reserve the canvas dimensions before initialization so loading does not shift the surrounding UI.
- Show which asset or stage is loading when progress is meaningful; a permanent spinner without recovery is not a loading state.
- Provide an actionable fallback for WebGL, shader, asset, or capability failure. The fallback can be a static image, inspectable object view, or equivalent product task surface.
- Keep camera framing, controls, and overlay state stable across resize and route changes.
- Cap pixel ratio, dispose unused assets, and pause or reduce animation when the surface is hidden or reduced motion is requested.
- Test the scene at desktop and mobile aspect ratios; a nonblank canvas alone does not prove correct framing or usable controls.
