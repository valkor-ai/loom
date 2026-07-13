# Playwright Delivery Core

Use the MCP-derived browser verification profile as the scope authority. This reference explains how to turn its checks into maintainable browser automation without broadening the task or replacing the project's test stack.

## Verification Design

Start from the behavior the check must prove:

1. Identify the assigned `verificationId`, viewport, backend mode, task-owned workflow, and UI surface.
2. Choose the shortest user-observable path that reaches the required outcome.
3. Arrange only the state that path needs.
4. Perform actions through visible controls or declared browser navigation.
5. Assert the business outcome, local UI state, and relevant persistence or API effect.

A browser check is not a tour of the application. Do not add unrelated navigation, full regression coverage, or every breakpoint just because Playwright can reach them.

## Test Layer Boundary

Use Playwright for behavior that requires a browser boundary:

- route entry, navigation, history, refresh, deep links, and browser storage;
- user workflows crossing components, providers, and API calls;
- rendered responsive layout and viewport-specific interaction;
- keyboard/focus behavior and browser-level semantics;
- integration feedback such as submitting, success, validation, and business blocking.

Keep pure functions, reducers, composables, hooks, isolated component states, and server-only rules in their existing unit or integration test layer. Do not move cheap deterministic checks into a browser suite.

## Project Adaptation

- Reuse the repository's package manager, Playwright config, test roots, scripts, fixtures, and naming conventions when present.
- Follow the request-selected project runner and shared-runtime contract; this reference does not override runner selection or dependency ownership.
- When no Playwright project exists and the task owns suite setup, create the smallest config and test root that support the assigned checks.
- Do not install a second E2E runner beside an accepted existing browser test stack unless the technical baseline selected Playwright for the new project.

## Check Anatomy

Use test names that state behavior and outcome:

```typescript
test('saved profile name survives a browser reload', async ({ page }) => {
  await page.goto('/account/profile');
  await page.getByLabel('Display name').fill('River Team');
  await page.getByRole('button', { name: 'Save changes' }).click();

  await expect(page.getByRole('status')).toContainText('Profile updated');
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Account profile' })).toBeVisible();
  await expect(page.getByLabel('Display name')).toHaveValue('River Team');
});
```

The example asserts the visible state transition. It does not assert internal component names, CSS classes, or implementation state.

## Backend Mode

- `real`: run the assigned workflow against the real project backend and persistence boundary. Seed controlled data through an existing fixture/API path. Do not replace the central success path with `page.route()` mocks.
- `not_applicable`: keep the check rendered and deterministic without inventing a backend. Static or client-only surfaces may use local fixture data only when that matches production behavior.

Network interception may control a specific failure or timing condition. It must not silently convert a real-backend check into a mocked component demonstration.

## State And Isolation

- Each test creates or identifies its own records; never depend on another test's execution order.
- Use stable unique values for mutable records and clean them through supported project fixtures when cleanup matters.
- Keep authentication state scoped by role and environment. Do not commit credentials or generated auth state.
- Parallelize only after state ownership is isolated. `fullyParallel: true` is not a quality signal when tests mutate shared records.
- Avoid hidden prerequisites. A failing setup must identify the missing service, credential, seed, or route.

## Assertions

Prefer web-first assertions that retry against observable state:

```typescript
await expect(page.getByRole('button', { name: 'Save' })).toBeEnabled();
await expect(page.getByRole('status')).toHaveText('Saved');
await expect(page).toHaveURL(/\/account\/profile$/);
```

Do not use `waitForTimeout()` as synchronization. Do not use `networkidle` as a generic readiness condition in applications with polling, analytics, sockets, or background refresh. Wait for the response, URL, element state, or business outcome that actually gates the next action.

## Artifact Discipline

- Keep traces, screenshots, videos, and reports in the project's configured output directory.
- Retain diagnostic artifacts on failure or retry according to configuration; do not paste their contents into TaskResult summaries.
- A retry success remains a retry success. Preserve the attempt count and investigate repeated instability instead of reporting a clean first-pass result.
- If environment preparation fails, record the concrete missing dependency or service. Do not relabel an environment failure as a product defect.

A check is complete only when the assigned viewport and backend mode ran, the expected business outcome was observed, the invocation returned control, artifacts are referenced rather than embedded, and no temporary server or browser process is left unmanaged.
