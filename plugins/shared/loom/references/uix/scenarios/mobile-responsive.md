# UIX Scenario: Mobile Responsive Web

Use when a web product must work well on phones and tablets without being a native app.

## Baseline

- Mobile behavior is a first-class layout, not a shrunken desktop.
- Density is usually `comfortable`.
- All critical actions must be reachable by touch and keyboard.
- Safe areas, sticky bars, and browser viewport changes must be considered.

## Required Patterns

- Mobile navigation: drawer, bottom nav, compact tabs, or simplified topbar.
- Forms: single-column, visible labels, correct input types, validation near fields.
- Tables: convert to cards/detail routes or allow scoped horizontal scroll only where data comparison requires it.
- Primary action: visible and safe-area-aware, but not covering content.
- Feedback: inline plus toast when appropriate.

## Layout

- Use mobile-first CSS and upgrade at breakpoints.
- Avoid fixed desktop widths.
- Use `min-height: 100dvh` or existing equivalent when viewport height matters.
- Keep tap targets large enough and separated.

## Avoid

- Hover-only menus.
- Horizontal page scroll.
- Sticky headers/footers that consume too much of the viewport.
- Truncated labels with no way to inspect full content.
