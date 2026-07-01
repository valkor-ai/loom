# UIX Scenario: Immersive 3D

Use for Three.js/WebGL/canvas scenes, product configurators, spatial tools, games, simulations, and immersive interactive visuals.

## Baseline

- The primary scene is full-bleed or dominant, not trapped inside a decorative card.
- The scene must render nonblank, be correctly framed, and respond to expected interaction.
- Density is `immersive`.
- Controls must support the scene task without covering critical visual content.

## Required Patterns

- Stable canvas sizing and resize handling.
- Loading and fallback state for assets/shaders/WebGL support.
- Scene controls: camera, zoom, rotate, reset, selection, mode toggle, or inspector as required.
- Overlay UI with readable contrast and safe placement.
- Reduced-motion or performance fallback when practical.

## Layout

- Use full viewport or dominant scene area with docked controls.
- Keep menus/panels collapsible or positioned outside the core focal area.
- Mobile: test touch gestures and avoid tiny overlay controls.

## Verification

- Verify canvas pixels are nonblank.
- Check desktop and mobile framing.
- Confirm referenced assets load.
- Confirm animation/interaction continues after initial render.

## Avoid

- Static placeholder canvas.
- Dark blurred background with no inspectable object.
- Controls that occlude the subject or cannot be used by touch.
