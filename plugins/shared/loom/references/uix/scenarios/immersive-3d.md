# Immersive 3D UIX

Use for Three.js, WebGL, canvas-heavy experiences, 3D product views, simulations, games, and immersive visual tools.

## Baseline

- The primary scene should be visible, nonblank, framed correctly, and interactive when interaction is part of the product.
- UI controls should not obscure essential scene content and should remain usable across desktop and mobile.
- Loading, asset failure, reduced-motion, and low-performance fallback states are product requirements, not optional polish.

## Required Patterns

- Verify scene render with screenshot or pixel-level evidence when possible.
- Provide camera/reset controls or predictable navigation when the scene can move.
- Use progressive loading or clear progress for heavy assets.
