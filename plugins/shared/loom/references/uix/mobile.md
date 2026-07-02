# UIX Focus: Mobile

Load this when a web surface must be responsive, when a mobile/native scenario is selected, or when the task changes touch behavior.

## Baseline

- Mobile is not a squeezed desktop.
- One primary task per screen is the default.
- Touch targets are comfortable and separated.
- Sticky top/bottom bars respect safe areas and do not hide content.
- Hover-only behavior is invalid.

## Layout

- Use single-column task flow for forms and details.
- Convert dense tables into list/detail cards or drill-down routes unless comparison truly requires horizontal table scroll.
- Keep primary action visible near the end of the task or in a safe sticky region.
- Collapse sidebars to drawers, rails, or bottom navigation.
- Use mobile viewport units and safe-area padding when full-height screens are used.

## Breakpoint Behavior

| Width | Expected behavior |
| --- | --- |
| `< 640px` | Single-column task flow, bottom or inline actions, no hover dependency. |
| `640-767px` | Wider cards or two-up secondary content only when it does not crowd controls. |
| `768-1023px` | Tablet split view may appear; drawers replace fixed sidebars. |
| `>= 1024px` | Desktop scenario baseline may apply. |

Do not rely on CSS variables inside media query conditions; write literal breakpoint values and keep the token names as documentation or framework config.

## Mobile Skeleton

```css
.mobile-task {
  min-height: 100dvh;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
}

.mobile-task__content {
  min-width: 0;
  overflow: auto;
  padding: var(--space-4);
}

.mobile-task__actions {
  padding: var(--space-3) var(--space-4);
  padding-bottom: calc(var(--space-3) + env(safe-area-inset-bottom, 0));
  border-top: 1px solid var(--border);
  background: var(--surface-raised);
}
```

Use this structure for mobile forms, record details, checkout/review flows, and app-like responsive pages.

## Data And Navigation Fallbacks

- Table to cards: keep title, status, key facts, and primary row action visible.
- Sidebar to drawer: preserve active section and provide close/back control.
- Detail panel to route/sheet: preserve selected record and return path.
- Topbar to compact header: keep page title and one primary command visible.
- Pagination to load-more or compact pager when page numbers do not fit.

## Inputs

- Use correct input types for number, email, phone, date, search, and password.
- Keep labels visible.
- Place validation next to the field.
- Preserve values when the keyboard opens/closes or validation fails.

## Verification

- Check narrow viewport, keyboard behavior, scroll, touch targets, and sticky bars.
- Check long labels and business messages in the target language.
- Check that error and success feedback remain visible after submit.
- Check that drawers/sheets close without losing form or selection context.
- Check 200% zoom or system text scaling when the environment allows it.
