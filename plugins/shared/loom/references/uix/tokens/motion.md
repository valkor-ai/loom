# UIX Token: Motion

Load this file when adding animation, transitions, skeletons, drawers, modals, route transitions, charts, 3D scenes, or feedback states.

## Purpose

Motion must explain state or preserve orientation. It should not decorate a static interface.

Use motion for:

- Opening and closing drawers, sheets, menus, popovers, and modals.
- Showing loading progress or skeleton transitions.
- Confirming success, undo, or destructive actions.
- Revealing details after user intent.
- Preserving spatial continuity in mobile navigation or 3D controls.

Avoid motion for:

- Spinning logos, bouncing icons, flashing borders, parallax by default, or decorative loops.
- Frequent table row changes that distract from scanning.
- Slow transitions that block repeated work.

## Timing

- Micro feedback: 100-160ms.
- Hover/focus/pressed: 120-180ms.
- Drawer/menu/popover: 160-240ms.
- Modal/sheet: 180-280ms.
- Route or large panel transition: 220-360ms.
- Loading skeleton shimmer: subtle and optional; prefer stable skeleton blocks.

## CSS Token Skeleton

```css
:root {
  --duration-fast: 120ms;
  --duration-normal: 180ms;
  --duration-slow: 280ms;
  --ease-standard: cubic-bezier(0.2, 0, 0, 1);
  --ease-exit: cubic-bezier(0.4, 0, 1, 1);
  --ease-enter: cubic-bezier(0, 0, 0.2, 1);
}

@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 1ms !important;
    transition-duration: 1ms !important;
    scroll-behavior: auto !important;
  }
}
```

## Implementation

- Animate `transform` and `opacity` first.
- Avoid animating layout properties such as `width`, `height`, `top`, `left`, or `margin` in frequent interactions.
- Use easing that decelerates naturally; avoid elastic/bounce unless the product tone explicitly supports it.
- Respect `prefers-reduced-motion`; provide instant or near-instant alternatives.
- Keep animation definitions close to design tokens or existing animation utilities.
- Loading skeletons should preserve layout dimensions; animation is secondary to stable structure.
- Long-running operations should expose progress or pending state at the region that is waiting, not through a global decorative spinner.

## Self-Check

- Motion makes the user's next state easier to understand.
- Reduced-motion users can complete the same workflow.
- Long-running operations show progress without blocking the page.
- No animation causes layout jump, scroll jump, or visual overlap.
- Motion evidence notes reduced-motion support when new transitions or animations were added.
