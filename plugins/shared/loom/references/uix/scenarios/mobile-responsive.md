# UIX Scenario: Mobile Responsive Web

Use when a web product must work well on phones and tablets without being a native app. Mobile is a first-class layout, not a shrunken desktop.

## Baseline

- Density is usually `comfortable`.
- Critical actions must be reachable by touch and keyboard.
- Safe areas, sticky bars, and browser viewport changes must be considered.
- Hover-only behavior is invalid.

## Mobile Layout Skeleton

```css
.mobile-page {
  min-height: 100dvh;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  background: var(--surface);
}

.mobile-topbar {
  position: sticky;
  top: 0;
  z-index: var(--z-sticky);
  min-height: 56px;
  padding: env(safe-area-inset-top, 0) var(--space-4) 0;
  border-bottom: 1px solid var(--border);
  background: var(--surface-raised);
}

.mobile-content {
  min-width: 0;
  overflow: auto;
  padding: var(--space-4);
}

.mobile-actionbar {
  padding: var(--space-3) var(--space-4);
  padding-bottom: calc(var(--space-3) + env(safe-area-inset-bottom, 0));
  border-top: 1px solid var(--border);
  background: var(--surface-raised);
}
```

## Required Patterns

- Mobile navigation: drawer, bottom nav, compact tabs, or simplified topbar.
- Forms: single-column, visible labels, correct input types, validation near fields.
- Tables: convert to cards/detail routes or allow scoped horizontal scroll only where data comparison requires it.
- Primary action: visible and safe-area-aware, but not covering content.
- Feedback: inline plus toast when appropriate.

## Responsive Upgrade

```css
@media (min-width: 768px) {
  .responsive-workspace {
    max-width: 720px;
    margin: 0 auto;
    padding: var(--space-6);
  }
}

@media (min-width: 1024px) {
  .responsive-workspace.is-operational {
    max-width: none;
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr) 320px;
    gap: var(--space-6);
  }
}
```

## Mobile Interaction

- Touch targets should be at least 44px.
- Use bottom sheets for short secondary flows; use full-screen routes for complex forms.
- Keep keyboard-open behavior usable for forms.
- Use `100dvh` for app-like full-height pages.
- Preserve scroll position when closing drawers/sheets where possible.

## Web-Specific Checks

- Viewport meta must not disable user zoom.
- Sticky headers/action bars must not cover focused inputs when the software keyboard opens.
- Scroll locking for drawers/sheets must release correctly.
- Table/list/detail fallbacks should preserve the same business actions as desktop.
- Desktop-only hover affordances need visible mobile equivalents.

## Verification Signals

- Check at least one narrow viewport and one desktop/tablet viewport when responsive behavior is in scope.
- Long labels, validation messages, and business-blocking copy wrap without hiding actions.
- Primary action remains reachable after scrolling and after validation errors.

## Avoid

- Horizontal page scroll.
- Fixed desktop widths.
- Font sizes below 16px for primary mobile inputs.
- Desktop-style centered modals for important mobile actions.
- Disabling user zoom.
