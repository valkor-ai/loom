# Playwright Reliability And Repair

Load this reference for failed, blocked, retried, inconsistent, or flaky browser checks. The objective is to preserve the original signal, identify its class, and make the smallest justified repair.

## Preserve The Signal

- Keep the original check id, command, attempt count, observed outcome, and artifact refs.
- A pass after retry is not equivalent to a first-attempt pass.
- Do not delete a failing assertion, add a broad catch, mark a test skipped, or raise global retries to make delivery green.
- Do not rewrite failed or blocked evidence as passed to satisfy TaskResult validation.
- Inspect compact check evidence first; open trace/report/screenshot artifacts only when the cause remains ambiguous.

## Failure Classification

| Class | Typical signal | Repair boundary |
| --- | --- | --- |
| Product defect | wrong visible state, request, navigation, focus, layout, or persistence outcome | application code, then rerun assigned check |
| Test defect | stale locator, wrong seed, assertion outside accepted behavior, leaked route handler | test/fixture/config only |
| Environment blocker | browser missing, service unavailable, credentials absent, unsupported host capability | prepare/fix environment or report concrete blocker |
| Flaky synchronization | intermittent race, animation, eventual response, non-isolated data | causal wait, stable assertion, isolation fix |
| Contract gap | assigned check cannot be performed inside task/runtime boundary | route to planning/architecture contract repair; do not invent evidence |

Classify from evidence. A timeout alone does not identify the class.

## Environment Route

Browser installation and launch failures are not test retries and are not product-code repair work.

Use `blocked` only when the supplied browser execution environment cannot launch or run. A reachable browser that observes application startup, API, selector, assertion, state, or workflow failure is `failed` product evidence and remains eligible for execution repair.

1. MCP runs host integrity and launch smoke checks.
2. Host launch failure triggers the exact-version managed-container smoke automatically.
3. If both fail, Loom records blocked browser checks and proceeds to Review without creating an execution-repair task.
4. The user gate offers only `retry_browser_environment`, `submit_external_browser_evidence`, and `approve_quality_waiver`.

Retry only after the environment, container runtime, registry access, or system dependencies changed. External evidence must cover every required check id with a concrete report/artifact and observed outcome. A waiver records accepted missing evidence; it does not rewrite blocked checks as passed.

## Diagnostic Order

1. Read the check status, attempts, command, and observed outcome.
2. Confirm the expected viewport, backend mode, base URL, and project runner.
3. Reproduce the single check once with the same environment.
4. Inspect the trace timeline: navigation, actions, network, console, DOM snapshots, and assertion target.
5. Inspect screenshot/video only for visual state that the trace does not settle.
6. Compare the locator and expected outcome with current product semantics and API contract.
7. Repair one classified cause and rerun the same check.
8. Use repeated execution only after the original check passes once.

Do not start by rerunning the entire suite or increasing timeout.

## Interactive Diagnostics

Use local diagnostics only after the focused check reproduces:

```bash
pnpm playwright test path/to/check.spec.ts --debug
pnpm playwright test path/to/check.spec.ts --headed
pnpm playwright show-trace test-results/.../trace.zip
```

UI mode is useful for local exploration when the project supports it. Remove committed `page.pause()`, slow motion, always-on traces, and temporary console dumping after diagnosis. When browser console or page errors are relevant, capture them narrowly and attach a concise failure summary rather than streaming unbounded output:

```typescript
const pageErrors: string[] = [];
page.on('pageerror', error => pageErrors.push(error.message));
// Perform the assigned workflow.
expect(pageErrors).toEqual([]);
```

## Synchronization Repairs

Replace fixed sleeps with the event that makes the next step valid:

```typescript
const response = page.waitForResponse(r =>
  r.url().endsWith('/api/account/profile') && r.request().method() === 'PATCH'
);
await page.getByRole('button', { name: 'Save changes' }).click();
expect((await response).status()).toBe(200);
await expect(page.getByRole('status')).toContainText('Profile updated');
```

Use locator actionability and web-first assertions. Avoid `networkidle` for apps with continuous traffic. Wait for URL only when navigation is the expected transition.

## Locator Repairs

- If strict mode finds multiple matches, scope to the business region/record.
- If a role/name is missing because the UI is non-semantic, repair the UI when in task scope.
- If copy intentionally changed, update the locator and assertion to the accepted product language.
- Do not replace a meaningful locator with `.first()`, XPath, generated classes, or `force: true`.

## Isolation Repairs

- Generate unique mutable record keys per worker and retry.
- Reset mocks, routes, storage, clock, and test data through owned fixtures.
- Do not share page/context instances across tests.
- Make cleanup idempotent and record-specific.
- If the product enforces global uniqueness or queue ordering, use namespaced state or a narrowly serial group with the dependency documented.

## Network Repairs

- Start response waits before triggering actions.
- Match method and endpoint, not a broad substring.
- Ensure a route mock does not intercept unrelated calls.
- For `backendMode: real`, restore the real central path; keep mocks only for the assigned edge condition.
- Separate slow backend startup from UI assertion timeout.

## Visual Reliability

- Wait for fonts, critical media, and deterministic data before capture.
- Disable animations/caret for snapshots without changing application logic.
- Pin browser, viewport, locale, timezone, and color scheme used by the baseline.
- Mask only genuinely variable values.
- Do not update snapshots until the rendered change is reviewed as intended.

## Retry Policy

Retries capture diagnostics and reveal instability; they are not the repair.

- Keep local retries at zero during diagnosis.
- Preserve `attempts > 1` in evidence after a retry success.
- After a fix, repeat the focused check enough times to exercise the former race; avoid a fixed universal count when the risk differs.
- Repeated failure with the same signature and no progress is a stop condition, not a reason for endless reruns.
- If repeated runs expose a real intermittent product defect, keep it a product defect.

## Timeout Policy

Increase a timeout only when the accepted operation legitimately takes longer and the check already waits on the correct condition. Prefer a scoped assertion, navigation, action, or server-start timeout. A global timeout increase can hide every unrelated regression.

- Measure whether delay belongs to build/startup, navigation, action completion, or assertion convergence.
- Keep the increase local to that boundary and retain a failure message that identifies the unmet condition.
- Do not combine a timeout increase with extra retries before proving which change fixed the signal.

## Artifact Handling

- Keep trace, screenshot, video, and report files as refs.
- Avoid copying full logs or trace content into repair context.
- Redact or avoid secrets and sensitive payloads.
- Retain artifacts from the failing/retried attempt until the repair is reviewed.
- Successful first-pass checks usually need counts/refs, not artifact inspection.

Repair is complete only when the same assigned check passes with the intended viewport/backend mode, the attempt history remains truthful, the root cause is removed rather than suppressed, and no unmanaged runtime process remains. If the supplied runtime becomes unavailable during the closure task, record the specific blocker once. Do not reinstall browsers, rerun the same launch command, mark the product task failed, or enter generic execution repair.
