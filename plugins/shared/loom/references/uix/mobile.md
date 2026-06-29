# Mobile UIX

Load this file for mobile, tablet, responsive web, PWA, React Native, Expo, Flutter, iOS, or Android work.

## Targets

- Identify the required device classes, orientation expectations, and minimum supported viewport width before implementation.
- Use realistic mobile content density. Avoid desktop tables, hover-only actions, wide multi-column controls, and tiny hit areas on touch-first screens.
- Respect safe areas, browser chrome, virtual keyboard behavior, scroll containment, and sticky headers/footers.

## Interaction

- Keep primary actions reachable without hiding essential context.
- Use 44 CSS px or platform-equivalent touch targets for frequent or risky actions unless the existing design system has stricter rules.
- Provide focus, pressed, disabled, loading, validation, and retry states for touch flows.
- Ensure drawers, sheets, menus, tabs, filters, search, and date/number inputs can open, close, and recover cleanly.

## Verification

- Check at least one narrow mobile viewport and one desktop/tablet viewport when the workflow is responsive.
- Verify text wrapping, overflow, sticky controls, modals/sheets, keyboard-driven input, and long content.
- Record mobile evidence in the TaskResult when mobile support is required or changed.
