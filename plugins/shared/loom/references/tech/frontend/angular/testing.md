# Angular Testing

Use Angular framework testing only for tasks that own tests. Choose the smallest proof that executes the changed boundary. Playwright/browser references remain authoritative for rendered multi-viewport, real navigation, and end-to-end business-flow evidence when MCP assigns a browser profile.

## Proof Boundary

| Claim | Preferred proof |
|---|---|
| Pure mapper/validator/selector/reducer | Plain TypeScript unit test |
| Service/DI/interceptor | TestBed/injection test |
| HTTP adapter contract | Angular HTTP testing backend |
| Component rendering/interaction | TestBed component test or selected harness/library |
| Guard/resolver/navigation | RouterTestingHarness/injection test |
| RxJS timing/order | TestScheduler/marble test |
| Full rendered workflow/viewport | Assigned Playwright browser task |

Do not use component TestBed for every pure function or claim a unit DOM test proves browser layout, CSS, focus across overlays, deep-link refresh, or frontend/backend deployment binding.

## TestBed And Standalone Components

Import standalone components directly and provide only required collaborators. Preserve the real providers/pipes/directives when their behavior is part of the claim; do not shallow-mock away the failing integration.

Set signal inputs through supported fixture/component APIs and trigger the real DOM event/service response that updates state. Avoid mutating private component fields to manufacture the expected view.

Use accessible roles, labels, names, text, or established stable test IDs. Assertions should cover visible output, enabled/disabled state, emitted intent, focus, and recovery rather than CSS implementation classes alone.

## HTTP Services And Interceptors

Use the repository's Angular-version-compatible HTTP test providers (for example `provideHttpClient()` with `provideHttpClientTesting()`) or existing module setup. Verify outstanding requests after each test.

Assert exact method, URL/base path, params, headers, body, credentials behavior, response mapping, and error mapping. Cover cancellation/deduplication only when the service owns it.

Do not duplicate backend behavior in mocks. Return contract-shaped success and error responses, including validation/conflict/permission/unavailable cases used by the UI.

## Routing, Guards, And Resolvers

Use `RouterTestingHarness` for route activation, redirects, params, query values, guards, resolvers, and component input binding. Use `TestBed.runInInjectionContext` for a focused functional guard/resolver only when route integration is not the claim.

Test allowed and blocked navigation, not just the boolean branch. Deep-link refresh and server fallback remain browser/runtime evidence.

## RxJS And Async Behavior

Use `TestScheduler.run`/marbles when virtual time, cancellation, ordering, debounce, retry, or concurrency is the risk. For straightforward finite streams, direct subscription/firstValueFrom tests are clearer.

Use `fakeAsync`/`tick` only for code using compatible Zone-managed timers and flush pending work. Do not mix fake timers, real sleeps, unresolved promises, and marble schedulers in one test.

Assert loading/disabled cleanup on success, error, and cancellation. Verify subscriptions/resource cleanup on fixture destruction when lifecycle is task-owned.

## NgRx Tests

Test reducers/selectors as pure functions. Test effects with controlled action and service streams, asserting emitted actions and operator semantics. Use `provideMockStore` for components/facades, not as the only proof of store implementation.

Override selectors before initial change detection and reset them between tests. Avoid giant initial-state fixtures that couple unrelated features.

Feature registration/key, lazy effects, persisted state, and router-store integration need an integration boundary beyond isolated reducer/effect tests.

## Component Workflow States

Cover only task-owned states but include meaningful transitions: loading to ready/empty/error, edit draft to validation/submitting/success, conflict/permission failure to recovery, and destructive confirmation/cancel.

For repeated list actions, prove the displayed target ID survives sorting, filtering, pagination, refresh, and modal open/close. Verify backend field errors are associated with controls and stale errors clear appropriately.

## Verification And Cleanup

Run the changed test file/project first using the repository's runner (Karma/Jasmine, Jest, Vitest, or another configured target). Then run focused build/typecheck/lint when templates, public types, routes, providers, or shared state changed. Do not impose a universal coverage percentage absent a repository requirement.

Destroy fixtures, verify HTTP requests, restore timers/selectors/global state, and close any custom resources. Flaky ordering or leaked subscriptions are defects, not reasons to add sleeps.

## Delivery Evidence

Record the test boundary, scenario, command, and meaningful visible/contract assertion. A passing suite count, private-field assertion, or coverage number alone cannot prove route integration, HTTP contract, reactive cancellation, accessibility, responsive rendering, or browser flow closure.

## Unsafe Defaults

- Angular testing guidance loaded for implementation-only tasks.
- Private state mutated instead of exercising public behavior.
- HTTP tests checking only that one request occurred.
- Direct guard tests claimed as route behavior evidence.
- `fakeAsync`, marbles, and real sleeps mixed indiscriminately.
- Mock store replacing reducer/effect correctness tests.
- Universal coverage thresholds copied from an external skill.
