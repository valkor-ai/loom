# Playwright Locators And Assertions

Locators are part of the UI contract. They should describe what a user or assistive technology can identify, survive visual refactors, and fail clearly when the product becomes ambiguous.

## Locator Priority

Use this order unless the existing project has a stricter convention:

1. `getByRole()` with an accessible name.
2. `getByLabel()` for form controls.
3. `getByPlaceholder()` only when placeholder text is a real stable cue.
4. `getByText()` for stable business copy or status text.
5. `getByTestId()` for non-semantic visualization, virtualized content, or a stable integration anchor.
6. CSS or XPath only at a third-party or legacy boundary that cannot expose a better contract.

```typescript
const save = page.getByRole('button', { name: 'Save changes' });
const amount = page.getByLabel('Approved amount');
const status = page.getByRole('status');
```

If a semantic locator cannot find an ordinary button, field, link, heading, dialog, table, or alert, first inspect the product semantics. Adding a test id must not hide an accessibility defect.

## Scope Before Position

Resolve ambiguity by narrowing to a meaningful region or record:

```typescript
const members = page.getByRole('region', { name: 'Workspace members' });
const row = members.getByRole('row', { name: /jamie@example\.com/ });
await row.getByRole('button', { name: 'Edit role' }).click();

const dialog = page.getByRole('dialog', { name: 'Edit member role' });
await dialog.getByLabel('Role').selectOption('editor');
await dialog.getByRole('button', { name: 'Save role' }).click();
```

Use `filter({ hasText })` or `filter({ has })` for repeated records. A row id, stable business key, or named region is stronger than `.first()` or `.nth()`.

## Exactness Rules

- Use `{ exact: true }` when nearby labels intentionally share words and the exact product label is stable.
- Use a narrow regular expression for generated identifiers or localized suffixes, not a broad case-insensitive expression that can match unrelated content.
- Do not bind to timestamps, random ids, translated copy, or generated class names unless that value is the behavior under test.
- Prefer a stable business identifier over sample person's name when records can collide.

```typescript
await expect(page.getByText('Approved', { exact: true })).toBeVisible();
await expect(page.getByRole('heading', { name: /^Order ORD-\d+$/ })).toBeVisible();
```

## Forms

- A visible label should resolve the associated control.
- Scope repeated field labels to their form, fieldset, row, or dialog.
- Assert validation near the field or through its alert/status relationship.
- Use realistic input methods: `fill`, `press`, `selectOption`, `check`, and `setInputFiles`.
- Do not mutate DOM values with `evaluate()` to bypass the real control contract.

```typescript
const form = page.getByRole('form', { name: 'Create workspace' });
await form.getByLabel('Workspace name').fill('Northwind Studio');
await form.getByLabel('Region').selectOption('eu-west');
await form.getByRole('button', { name: 'Create workspace' }).click();
await expect(form.getByRole('alert')).toContainText('Plan is required');
```

## Tables, Lists, And Virtualized Data

- For semantic tables, locate the row by its accessible row name, then locate cells/actions inside it.
- For card/list layouts, locate a list item by the business key and scope actions to that item.
- For virtualized data, scroll through the component's public behavior. A test id on the virtualized viewport is acceptable when rows are not represented semantically.
- Never click an unscoped `Edit`, `Delete`, or overflow button in repeated content.

## Menus, Dialogs, Drawers, And Portals

Portaled content may not be a DOM descendant of its trigger. Scope by semantic overlay role and accessible name, not by parent CSS:

```typescript
await page.getByRole('button', { name: 'More actions' }).click();
await page.getByRole('menu').getByRole('menuitem', { name: 'Archive' }).click();
await expect(page.getByRole('dialog', { name: 'Archive workspace' })).toBeVisible();
```

Verify focus enters modal content and returns to the trigger when that behavior is task-owned.

## Web-First Assertions

Use locator assertions instead of one-time value reads:

```typescript
await expect(locator).toBeVisible();
await expect(locator).toBeEnabled();
await expect(locator).toHaveText('Ready');
await expect(locator).toHaveAttribute('aria-current', 'page');
await expect(page).toHaveURL(/status=approved/);
```

`textContent()`, `isVisible()`, and raw element handles return snapshots and can race with rendering. Use them only when the raw value itself must be transformed or compared.

## Test ID Policy

A test id is appropriate for:

- canvas/WebGL roots and chart series with no native semantic node;
- virtualized containers or drag handles;
- stable cross-team integration anchors;
- controls whose visible label is dynamic but whose semantics cannot be made unique.

Name ids by product role, not styling or component implementation: `account-activity-timeline`, not `blue-panel-2`; `chart-revenue-series`, not `recharts-layer`.

## Disallowed Shortcuts

- No arbitrary sleeps before locating an element.
- No `.first()`, `.last()`, or `.nth()` to silence strict-mode ambiguity without proving order is the contract.
- No generated CSS classes, deeply nested selectors, or XPath through layout wrappers.
- No `force: true` to bypass covered, disabled, or unstable controls unless the test explicitly proves that low-level condition.
- No broad text locator that can pass against hidden, duplicated, or stale content.
