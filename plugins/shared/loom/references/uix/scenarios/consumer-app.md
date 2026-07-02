# UIX Scenario: Consumer App

Use for customer-facing web apps, portals, booking, commerce, content, learning, productivity, and service workflows.

## Baseline

- First viewport lets the user start or continue the product task.
- Density is usually `comfortable` or `balanced`.
- Navigation is task-oriented and recoverable.
- Visual design may be expressive, but workflow clarity wins.

## App Structure

```html
<main data-region="consumer-app">
  <header data-region="app-header"></header>
  <section data-region="current-task"></section>
  <section data-region="object-list-or-feed"></section>
  <section data-region="detail-or-checkout"></section>
</main>
```

```css
.consumer-shell {
  min-height: 100dvh;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
}

.consumer-content {
  width: min(100%, 1120px);
  margin: 0 auto;
  padding: var(--space-4);
}

@media (min-width: 768px) {
  .consumer-content {
    padding: var(--space-6);
  }
}
```

## Required Patterns

- Clear primary path and secondary actions.
- Account/session, saved state, or progress indicators when relevant.
- Forms with validation, input preservation, and clear success/error states.
- Empty states with helpful next action.
- Responsive behavior that supports touch and narrow content.
- Consistent cards/lists/details when browsing objects.

## Workflow Rules

- Multi-step flows need progress and back/cancel behavior.
- Detail pages expose primary action without hiding supporting information.
- Recommendations or related content must not hide the user's current task.
- Notifications and toasts should not be the only record of a completed action.

## Surface Patterns

- Home/task surface: resume current item, start primary task, or show relevant feed.
- Browse/list surface: searchable/filterable when volume requires it, with clear selected item path.
- Detail/action surface: primary action, supporting facts, related history, and recovery path.
- Account/settings surface: user-controlled preferences and status, not hidden behind marketing copy.

```html
<section data-region="task-surface">
  <header data-region="task-context"></header>
  <section data-region="task-body"></section>
  <footer data-region="task-actions"></footer>
</section>
```

## Verification Signals

- The first screen lets the user act or resume, not only read a product promise.
- Mobile layout keeps primary action, error, and success visible.
- Long user-generated content and empty states do not break cards/lists.

## Avoid

- Marketing-only first screen for an app task.
- Generic card grids that do not help the user act.
- Critical controls that appear only on hover.
