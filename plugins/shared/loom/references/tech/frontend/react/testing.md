# React Component And Hook Testing

Use this reference only when the task explicitly owns React test implementation. Keep component/hook verification here; an MCP-selected browser profile owns Playwright navigation, multi-viewport workflow evidence, network synchronization, and browser artifacts.

## Test Boundary

Choose the smallest public boundary that proves the task behavior: pure function, reducer, hook contract, component, provider-composed feature, or route integration. Do not render the whole application for a local formatter, and do not mock every child when composition is the behavior under test.

Use the repository's existing runner, DOM environment, Testing Library, assertion extensions, request mocking, and fixture conventions. Do not introduce Jest/Vitest/MSW or a second provider harness merely because an external example uses it.

Tests should fail for a visible or emitted contract regression, not a private refactor.

## User-Facing Queries

Prefer role plus accessible name, label, text, and async `findBy*` queries. These reflect how users and assistive technology discover controls. A missing accessible query often reveals a component semantics defect.

Use `queryBy*` for absence and `getBy*` for immediate presence. Use `findBy*` or `waitFor` for eventual outcomes; do not wrap synchronous assertions in arbitrary waits.

Test IDs are a last resort for elements without a meaningful semantic identity, not a substitute for labeling a button, field, dialog, row, or status.

## Interaction

Use the repository's `userEvent` setup for typing, tabbing, selecting, clicking, and form submission. Assert the visible result and exact emitted command target/payload.

For list/detail/action surfaces, include a case where sort, filter, selection, pagination, refresh, or modal state changes before the command. This catches handlers that read stale global selection rather than the displayed target.

Cover duplicate-submit blocking, draft preservation after errors, server-normalized readback, and disabled/forbidden behavior when those states are task-owned.

## Provider Harness

Build one test render helper that mirrors required router, query client, store, theme, i18n, auth, and feature-flag providers while allowing per-test overrides. Create isolated query/store/router instances for each test.

Do not conceal required inputs behind permissive global defaults. A component that requires tenant/auth/router context should fail clearly when the provider contract is absent.

Use representative route params and navigation history for route-aware components. Assert navigation outcomes rather than mocking the router hook until nothing real remains.

## Network And Async Behavior

Mock at the accepted API client/network boundary with the repository's approach. Keep request method, path, query, headers, and body expectations aligned with the API contract; do not mock the hook under test.

Model success plus meaningful validation, authorization, conflict, not-found, unavailable, or malformed response states owned by the task. Reset handlers and reject unexpected network calls.

Wait for visible outcomes instead of sleeping. Use fake timers only for timer-owned behavior and restore them after each test. For replaceable requests, prove an older completion cannot overwrite newer state.

## Hook Contracts

Test a custom hook through `renderHook` or a small consumer component and assert its public state/actions. Include dependency changes, cleanup, cancellation, errors, and retry where those are the reason the hook exists.

Run relevant hook tests under Strict Mode when setup/cleanup replay can expose resource duplication. Avoid asserting exact render/effect counts unless they are the explicit performance contract.

## State And Cache Isolation

Reset stores, query caches, local/session storage, timers, mocks, and singletons between tests. Include identity/tenant dimensions in fixtures so leaked state is observable.

For optimistic updates, test immediate presentation, success reconciliation, rollback, conflicting/out-of-order completion, and server-normalized data.

## Accessibility Assertions

Assert names, roles, descriptions, error association, disabled semantics, focus movement/restoration, and keyboard interactions for the changed controls. Snapshot markup cannot establish accessibility behavior.

Keep DOM snapshots small and intentional for stable generated structure. Prefer behavior assertions for forms, routes, async state, and component composition.

## Verification

- Run the narrow changed test target, then the repository typecheck/build when public types, exports, providers, or bundling changed.
- Prove success and at least one task-owned blocking/failure path for mutations and API-backed surfaces.
- Verify request shape, stable command target identity, returned readback, and no unexpected calls.
- Exercise provider isolation, async cleanup, and stale-result safety where applicable.
- Keep Playwright evidence separate unless the task also has explicit browser-verification ownership.

## Delivery Evidence

Name the public behavior, harness/network boundary, representative states, and assertion that would fail on regression. Passing tests without identifying what they prove are weak evidence; implementation-detail snapshots and mocked-away behavior prove even less.

## Unsafe Defaults

- React testing loaded for every frontend implementation task.
- A new test stack introduced despite established repository tooling.
- Test IDs preferred over accessible roles and labels.
- Providers, hooks, and network client all mocked in the same test.
- Arbitrary sleeps or unbounded `waitFor` used for synchronization.
- Shared query/store/router state leaking between tests.
- Browser workflow claims made from DOM component tests alone.
