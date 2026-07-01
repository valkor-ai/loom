# UIX Stack: UniApp And Mini-App

Use for UniApp, WeChat/Alipay mini-programs, H5/mobile hybrid targets, and similar cross-platform mobile surfaces.

## Structure

- Follow the target platform's page, component, store, and routing conventions.
- Keep platform-specific capabilities behind small adapters when multiple targets are in scope.
- Design for mobile task flows first; desktop web patterns should not leak into mini-app screens.

## Implementation Rules

- Respect safe areas, native navigation bars, tab bars, and platform gesture expectations.
- Use platform-compatible units and components according to the repo's existing stack.
- Keep forms single-column, touch-friendly, and explicit about validation.
- Avoid hover-only interactions and tiny table layouts.
- Handle loading, empty, error, permission, and business-blocking states on the page that triggers them.

## Verification

- Use the available mini-app/H5 preview target when present.
- Check safe areas, keyboard behavior, scroll, and platform permission flows.
