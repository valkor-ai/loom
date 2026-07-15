# Playwright Browser Accessibility Verification

Browser accessibility checks prove the task-owned semantic and interaction contract. Automated scans are useful supporting evidence; they do not replace keyboard, focus, name, role, state, and dynamic-feedback checks.

## Semantic Entry Checks

Verify that users and assistive technology can identify the surface:

- page title and primary heading describe the current context;
- landmarks distinguish navigation, main content, complementary regions, and banners where appropriate;
- controls expose correct roles, accessible names, values, checked/expanded/selected states, and disabled state;
- form fields have persistent labels and associated help/errors;
- tables expose headers and relationships needed to understand rows;
- icon-only controls have an accessible name and visible tooltip when meaning is not universal.

```typescript
await expect(page).toHaveTitle(/Account settings/);
await expect(page.getByRole('main')).toBeVisible();
await expect(page.getByRole('heading', { level: 1, name: 'Account settings' })).toBeVisible();
await expect(page.getByRole('button', { name: 'Save changes' })).toBeEnabled();
```

The ability to locate a control by role is also a testability signal. Do not add a test id to avoid correcting a missing name or wrong element type.

## Keyboard Workflow

For task-owned actions, verify:

- focus order follows the visual/task order;
- all actions can be reached and activated without a pointer;
- visible focus is not clipped or obscured;
- dialogs trap focus while open, close through supported keys, and return focus to the trigger;
- menus, tabs, comboboxes, grids, and disclosure controls use their expected key behavior;
- sticky headers, sheets, and overlays do not cover the focused element.

```typescript
await page.getByRole('button', { name: 'Invite member' }).focus();
await page.keyboard.press('Enter');
const dialog = page.getByRole('dialog', { name: 'Invite workspace member' });
await expect(dialog).toBeVisible();
await expect(dialog.getByLabel('Email address')).toBeFocused();
await page.keyboard.press('Escape');
await expect(page.getByRole('button', { name: 'Invite member' })).toBeFocused();
```

Use `press()` and focus assertions on the real workflow. Do not simulate keyboard support by calling click handlers through JavaScript.

## Forms And Errors

- Invalid submit moves focus to the first invalid field or a clear error summary according to product convention.
- Field errors are associated with their controls and remain visible long enough to act on.
- Submitting state is conveyed semantically, not only by color or animation.
- Preserved input remains available after server/business failure.
- Required, read-only, disabled, and invalid states match actual behavior.

```typescript
await page.getByRole('button', { name: 'Create account' }).click();
const email = page.getByLabel('Email address');
await expect(email).toBeFocused();
await expect(email).toHaveAttribute('aria-invalid', 'true');
await expect(page.getByText('Enter a valid email address')).toBeVisible();
```

## Dynamic Feedback

Loading, success, validation, error, and business-blocking feedback should be discoverable without moving focus unexpectedly.

- Use status/alert/live-region behavior appropriate to urgency.
- Avoid announcing every keystroke or background refresh.
- Preserve the user's location when lists update.
- For destructive confirmation, make action and consequence explicit.
- When a disabled action can become available, expose the reason through nearby content or description.

## Navigation And Context

- Route changes update title, heading, and current navigation state.
- Skip links or equivalent navigation are usable when repeated chrome makes them necessary.
- Browser back/forward and deep-link entry preserve meaningful focus/context for assigned workflows.
- Drawers and client-side route transitions do not leave focus on removed elements.

## Media, Charts, Canvas, And 3D

Verify task-owned alternatives: alt text, captions/transcripts, accessible names, keyboard controls, chart summaries, or equivalent data access. A canvas screenshot can prove rendering but not accessibility. Do not claim semantic coverage when only pixels were inspected.

- Check that media controls expose names, state, and keyboard operation.
- Check that charts communicate title, series/legend meaning, and a text or data alternative where the product requires one.
- Check that canvas/3D controls do not trap keyboard focus and that essential actions have non-pointer access.
- Record unsupported assistive behavior as a gap instead of treating an image comparison as semantic proof.

## Automated Scans

Use the repository's existing accessibility scanner when available. Scope scans to the task-owned route or region and run them after the relevant state is rendered. Do not install another scanner for one check unless the technical baseline selects it.

An automated scan cannot prove:

- sensible focus order;
- correct accessible names in business context;
- complete keyboard workflow;
- useful error language;
- screen-reader announcement timing;
- equivalent access to a complex visualization.

Treat scan violations as findings to inspect, not counts to suppress. Any exclusion must name the third-party/legacy boundary and remaining risk.

## Viewport And Zoom

When responsive accessibility is assigned, check narrow layout and enlarged content behavior. Controls and text must remain reachable without two-dimensional scrolling for normal application content, except where the product inherently requires a large canvas/table.

Verify that sticky controls do not cover focused content, text reflows without clipping, touch targets remain distinct, and zoom does not remove the only path to a primary action. Keep exceptions scoped to the actual large-format surface.

## Evidence

Record the semantic locator or interaction checked, keyboard/focus outcome, dynamic state, viewport, and automated scan result when used. Do not report "accessible" based solely on zero scanner violations.

For an unresolved issue, identify the affected control/workflow, input method, expected behavior, observed behavior, and whether the cause is product code, third-party content, or environment limitation.
