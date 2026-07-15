# Playwright Fixtures And Reusable Models

Fixtures should make state ownership explicit and keep tests readable. Reuse is valuable when it removes repeated setup or captures a stable product interaction; abstraction is harmful when it hides the behavior a check must prove.

## Choose The Smallest Reuse Unit

Use a plain helper when setup is local to one file. Use a fixture when multiple tests need the same lifecycle-managed dependency. Use a page or component object when a stable product surface has several repeated interactions.

Do not create a Page Object Model directory for a single short test. Do not place assertions, test data factories, API clients, navigation, and every page interaction into one base class.

| Need | Preferred form |
| --- | --- |
| One test creates one record | local helper |
| Multiple tests need isolated records | worker/test fixture or typed factory |
| Repeated interaction with a stable surface | page/component model |
| Shared authenticated role | storage-state project or role fixture |
| Global mutable database state | avoid; use namespaced data and controlled reset |

## Fixture Lifecycle

- Test-scoped fixtures own mutable page, context, record, and temporary-file state.
- Worker-scoped fixtures may own expensive immutable services or unique worker namespaces.
- Teardown must be safe after partial setup and must not delete another worker's records.
- Fixtures should expose meaningful domain capabilities, not raw bags of unrelated values.
- Keep fixture dependencies acyclic and visible in their parameter list.

```typescript
import { test as base, expect, type APIRequestContext } from '@playwright/test';

type WorkspaceRecord = { id: string; slug: string };
type Fixtures = {
  workspaceRecord: WorkspaceRecord;
};

export const test = base.extend<Fixtures>({
  workspaceRecord: async ({ request }, use, testInfo) => {
    const externalKey = `pw-${testInfo.workerIndex}-${testInfo.retry}-${Date.now()}`;
    const response = await request.post('/api/test-support/workspaces', {
      data: { externalKey, name: 'Northwind Studio' },
    });
    expect(response.ok()).toBeTruthy();
    const record = await response.json() as WorkspaceRecord;
    await use(record);
    await request.delete(`/api/test-support/workspaces/${record.id}`);
  },
});

export { expect } from '@playwright/test';
```

Use an existing test-support API only when the project already permits it. Do not ship insecure reset or seed endpoints in production code just to make a browser test convenient.

## Authentication

- Prefer project-supported test identities, local auth bypasses, or controlled login fixtures.
- Store generated auth state under ignored test output, never in source control.
- Separate roles into named projects or fixtures so a test cannot accidentally inherit admin privileges.
- Refresh state when expiry matters; do not mask expiry behavior with permanently valid tokens.
- For a login workflow task, test the real login interaction instead of preloading storage state.

```typescript
import { test as setup, expect } from '@playwright/test';

const reviewerAuthFile = 'playwright/.auth/reviewer.json';

setup('authenticate as reviewer', async ({ page }) => {
  await page.goto('/login');
  await page.getByLabel('Email').fill(process.env.E2E_REVIEWER_EMAIL!);
  await page.getByLabel('Password').fill(process.env.E2E_REVIEWER_PASSWORD!);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL('/work-queue');
  await page.context().storageState({ path: reviewerAuthFile });
});
```

Credentials come from the project environment. Missing credentials are an environment blocker, not a reason to hard-code secrets.

## Page And Component Models

A model should expose product language and stable interactions:

```typescript
export class WorkspaceSettings {
  constructor(private readonly page: Page) {}

  heading(name: string) {
    return this.page.getByRole('heading', { name });
  }

  async transferOwnership(email: string) {
    await this.page.getByRole('button', { name: 'Transfer ownership' }).click();
    const dialog = this.page.getByRole('dialog', { name: 'Transfer workspace ownership' });
    await dialog.getByLabel('New owner').fill(email);
    await dialog.getByRole('button', { name: 'Confirm transfer' }).click();
  }
}
```

Keep assertions in the test when they express the scenario outcome. A model may expose a status locator; it should not decide that every call must assert the same message.

Avoid model inheritance. Compose page-level and component-level models when navigation, table, dialog, or editor interactions are shared.

## Test Data

- Use domain-valid minimal records. A giant generic fixture obscures which fields matter.
- Generate unique keys for mutable data; keep readable display values where assertions need them.
- Build invalid input in the test that owns the validation rule instead of weakening a global factory.
- Never depend on production data, clock-sensitive existing records, or test ordering.
- Freeze or inject time only through existing project support when time is the behavior under test.

## Parallel Safety

Before enabling full parallel execution, verify:

- record keys are unique per worker;
- auth sessions do not mutate the same user state;
- file downloads use `testInfo.outputPath()`;
- cleanup targets ids created by the same fixture;
- shared rate limits, queues, and background jobs are controlled;
- tests do not reuse a singleton page or browser context.

Serial mode is acceptable for a genuinely ordered workflow, but it should be local to that group and documented by the product dependency. Do not make the entire suite serial to hide state leakage.

Fixture setup failures should state which prerequisite failed. Do not catch and replace all errors with a generic "setup failed" message. Attach ids or safe response summaries to test annotations when they help diagnose the failure, but do not emit secrets or full payload dumps.
