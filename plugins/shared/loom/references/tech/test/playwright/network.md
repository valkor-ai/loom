# Playwright Network And Backend Boundaries

Network control exists to synchronize with real behavior and to create specific edge conditions. It must not erase the backend contract the browser check is assigned to prove.

## Honor Backend Mode

When the profile declares `backendMode: real`:

- start or reuse the actual project backend through the repository's supported command;
- point the frontend at that backend through existing runtime configuration;
- use real persistence or the project's accepted test provider;
- seed data through supported fixtures, APIs, migrations, or test harnesses;
- assert the visible result and, when relevant, the accepted API or persistence effect.

Do not fulfill the central create/update/read request with a mock. That would prove only frontend rendering while claiming an integration workflow passed.

When backend mode is not applicable, do not invent a service. Keep client-only state deterministic and aligned with production behavior.

## Synchronize With The Causal Request

Create the response promise before the action that triggers it:

```typescript
const saveResponse = page.waitForResponse(response =>
  response.url().endsWith('/api/cart/items/SKU-1042') &&
  response.request().method() === 'PUT'
);

await page.getByRole('button', { name: 'Update quantity' }).click();
const response = await saveResponse;
expect(response.status()).toBe(200);
await expect(page.getByRole('status')).toContainText('Cart updated');
```

An HTTP 200 is not the complete outcome. Assert the visible or navigational result that matters to the user. Conversely, visible success without checking the assigned write request can miss a disconnected optimistic UI.

## Mocking Decision

Use request interception for a condition that is expensive, destructive, external, or hard to trigger deterministically:

- upstream timeout or unavailable third party;
- a precise validation/business error response already defined by the API contract;
- delayed response needed to inspect loading or duplicate-submit prevention;
- static client application with no backend in the accepted architecture;
- test of frontend response mapping where real-backend coverage exists elsewhere.

Do not mock:

- the main success path of a real-backend workflow;
- request payload shape that the browser check is expected to integrate;
- authentication when login/session behavior is the feature;
- persistence read-back when delivery requires stored state;
- every request through a broad `**/api/**` handler that lets unexpected calls pass unnoticed.

## Precise Interception

Match method and URL, validate the request, and fail unexpected variants:

```typescript
await page.route('**/api/subscriptions/plan', async route => {
  const request = route.request();
  if (request.method() !== 'PUT') {
    await route.abort('failed');
    return;
  }
  expect(request.postDataJSON()).toEqual({ plan: 'team' });
  await route.fulfill({
    status: 409,
    contentType: 'application/json',
    body: JSON.stringify({ code: 'PLAN_CHANGE_BLOCKED', message: 'Resolve the outstanding invoice first' }),
  });
});
```

Scope routes to one test and unregister long-lived handlers when a fixture reuses a page. Avoid wildcard handlers that accidentally intercept assets, health checks, or unrelated operations.

## Loading And Concurrency States

Use a controlled promise instead of a fixed delay when the test must hold a request:

```typescript
let release!: () => void;
const gate = new Promise<void>(resolve => { release = resolve; });

await page.route('**/api/catalog', async route => {
  await gate;
  await route.continue();
});

await page.goto('/catalog');
await expect(page.getByRole('status').filter({ hasText: 'Loading products' })).toBeVisible();
release();
await expect(page.getByRole('list', { name: 'Products' })).toBeVisible();
```

For submit actions, verify the control disables or otherwise prevents duplicate submission while the request is pending.

## Response Modification And HAR

`route.fetch()` may preserve the real response while changing one field for a narrow client condition. Record what was modified and why; never use it on a check intended to validate the untouched backend response.

HAR playback is appropriate for stable external dependencies with reviewed fixtures. Keep HAR files free of secrets and personal data. Do not enable automatic HAR updates in ordinary verification runs because it can silently bless changed responses.

## WebSockets, SSE, Polling, And Background Refresh

- Wait for the visible event or application state, not `networkidle`.
- Control clocks or event sources through existing test support when deterministic sequencing matters.
- Assert reconnect, stale-data, or offline feedback only when assigned to the task.
- Ensure polling and sockets are closed with the page/context and do not keep the command alive after verification.

## Failure Classification

- Connection refused, missing credentials, unavailable registry/service, and unstartable dependencies are environment blockers.
- Wrong request method, payload, path, response mapping, or missing feedback is a product/integration defect.
- A mock that no longer matches the accepted API is a test defect; update it only after comparing the current contract.

Record concise request identity, status, and user-visible outcome. Keep full bodies, headers, traces, and secrets out of TaskResult prose.
