# UIX Focus: Web Implementation

Use this when a user-visible browser surface is created or changed. This file covers DOM, CSS, browser behavior, and Web framework edge cases that make a UI feel production-ready instead of merely styled.

Do not use this file for native mobile screens, mini-program targets, or primary 3D scenes unless the task also includes a normal browser UI around them.

## Semantic Accessibility

- Use native elements first: `button` for actions, `a` or framework links for navigation, `label` for form labels, `table` for real tabular comparison.
- Do not use clickable `div` or `span` elements for primary controls.
- Icon-only controls need an accessible name and, when meaning is not universal, a tooltip or nearby text.
- Decorative icons should be hidden from assistive technology.
- Dynamic validation, save, delete, and load feedback should be announced in the affected region, not only as a disconnected toast.
- Headings must form a usable outline. A page with multiple regions still needs one clear top heading and meaningful region labels.
- Images need useful alternative text when they communicate content. Decorative media should not compete with the task.

Accessible control pattern:

```tsx
<button
  type="button"
  aria-label="Refresh orders"
  className="inline-flex h-9 w-9 items-center justify-center rounded-md focus-visible:outline-none focus-visible:ring-2"
>
  <RefreshCw aria-hidden="true" />
</button>
```

## Focus And Keyboard

- Every interactive control needs a visible focus state.
- Removing outlines is acceptable only when a visible replacement exists.
- Prefer focus-visible styling so pointer clicks do not create noisy rings.
- Compound controls such as search boxes, comboboxes, cards with actions, and editable table rows need focus-within treatment.
- Hover-only controls require keyboard and touch equivalents.
- Modals, drawers, sheets, and menus must preserve focus flow and provide an obvious exit path.

Focus pattern:

```css
.control {
  outline: none;
}

.control:focus-visible {
  box-shadow: 0 0 0 3px var(--focus-ring);
}
```

## Forms And Actions

- Inputs need stable `name` values and visible labels. Placeholder-only labels are not enough.
- Use input types and input modes that match the value: email, tel, url, number, decimal, search, and similar.
- Use autocomplete thoughtfully so browsers can help without filling the wrong field.
- Do not block paste in normal fields. Paste is part of accessibility and recovery.
- Submit controls should enter a submitting state only after submission starts; do not disable the primary path before the user can act.
- Field errors belong next to the field. Form-level summaries should link or guide back to the affected field.
- Failed submission must preserve user input and selection context.
- Destructive actions need confirmation, undo, or a clear recovery path based on severity.
- Warn about unsaved changes when navigation would discard meaningful user work.

Form resilience pattern:

```tsx
<label htmlFor="supplier-email">Supplier Email</label>
<input
  id="supplier-email"
  name="supplierEmail"
  type="email"
  autoComplete="email"
  aria-describedby="supplier-email-error"
/>
<p id="supplier-email-error" role="alert">
  Enter a valid supplier email address.
</p>
```

## Layout Resilience

- Long names, identifiers, table values, and user-provided text need wrapping, truncation, or reveal behavior.
- Flex children that contain text often need `min-width: 0` so truncation can work.
- Empty strings, empty arrays, missing optional values, and partial records must not collapse the layout.
- Data tables need horizontal overflow or a mobile card/detail fallback when comparison is not the main goal.
- Fixed-format controls, counters, toolbar buttons, and table rows should not resize when state text changes.
- Avoid unwanted page-level horizontal scrolling; fix the element that overflows instead of hiding all overflow by reflex.

Long-content pattern:

```css
.record-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: var(--space-3);
}

.record-title {
  min-width: 0;
  overflow-wrap: anywhere;
}
```

## Motion And Interaction

- Honor reduced-motion preferences for transitions and animations.
- Animate transform and opacity for frequent transitions. Avoid layout-property animation in normal product flows.
- Do not use catch-all transitions; list the properties that should move.
- Motion must be interruptible. The UI should respond if the user clicks, closes, scrolls, or changes selection mid-animation.
- Drag, resize, and gesture flows should avoid accidental text selection and should keep inactive regions inert when needed.
- Touch targets must be large enough, and mobile flows must not depend on hover.

Motion pattern:

```css
.drawer {
  transition-property: transform, opacity;
  transition-duration: 160ms;
}

@media (prefers-reduced-motion: reduce) {
  .drawer {
    transition-duration: 1ms;
  }
}
```

## Media And Browser Performance

- Content images need stable dimensions or aspect ratios to avoid layout shift.
- Below-the-fold media should avoid eager loading. First-viewport critical media should be prioritized through the project stack's normal mechanism.
- Large lists need pagination, virtualization, chunked rendering, or content-visibility treatment once the visible count can grow beyond a small operational list.
- Avoid reading layout measurements during render. Measure after paint only when CSS layout cannot solve the problem.
- Expensive controlled inputs need debouncing, local buffering, or framework-specific optimization.
- Fonts and remote assets should be loaded through the project's existing performance pattern.

List strategy pattern:

```text
small bounded list -> normal render
large operational list -> pagination or virtualization
append-only feed -> incremental loading with stable item identity
wide comparison table -> horizontal overflow with labeled scroll region
```

## Navigation, Locale, And Hydration

- Filters, tabs, pagination, selected records, and expanded panels should be restorable through URL state or equivalent navigation state when users reasonably share, reload, or return to the view.
- Navigation links must keep browser affordances such as open in new tab and copy link.
- Use locale-aware date, time, number, and currency formatting for user-facing values.
- Keep code tokens, brand names, ids, and product identifiers from being accidentally translated when the UI supports translation.
- In server-rendered stacks, avoid hydration mismatch from random values, current time, viewport-only values, or uncontrolled-to-controlled input transitions.
- Use client-only rendering escapes sparingly and only for values that genuinely cannot match across server and browser.

Formatting pattern:

```ts
const amount = new Intl.NumberFormat(locale, {
  style: 'currency',
  currency,
}).format(value);
```

## Evidence Checklist

Implementation evidence should show:

- Semantic controls, labels, focus behavior, and dynamic feedback source checks.
- Form metadata, error placement, submission state, and recovery behavior when forms are in scope.
- Long-content, empty-value, media, list-size, and layout-overflow handling.
- Reduced-motion handling for animated UI.
- Locale formatting and navigation-state handling when those values or flows are user-visible.
- Hydration-sensitive values checked in server-rendered Web stacks.

## Quality Gate Index

| Gate | Pass signal | Fail signal |
| --- | --- | --- |
| `web.semantic_accessibility` | Native control semantics, accessible names, visible focus, and scoped feedback announcements are present in changed browser UI. | Clickable non-controls, unlabeled icon buttons or fields, hidden focus, missing dynamic feedback region, or inaccessible media. |
| `web.form_and_state_resilience` | Forms/actions keep meaningful metadata, inline errors, recoverable input, safe submission state, and destructive-action recovery. | Placeholder-only labels, blocked paste, lost input after failure, generic errors away from fields, double-submit risk, or immediate destructive actions. |
| `web.runtime_layout_safety` | Long content, empty values, media sizing, large lists, reduced motion, locale formatting, hydration-sensitive values, and restorable state are handled where in scope. | Text breaks layout, empty data renders broken UI, media shifts layout, large lists render naively, motion ignores user preference, values are hardcoded, or reload loses expected state. |
