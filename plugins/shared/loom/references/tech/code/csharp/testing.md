# C# Test And Verification Design

## When To Use

Use this reference only for explicit C# test ownership outside an already selected ASP.NET Core testing boundary, or for C# library/worker/CLI/Blazor tests needing language/runtime guidance.

## Implementation Focus

### Framework And Boundary

Follow the repository's xUnit, NUnit, MSTest, TUnit, SpecFlow, snapshot, property, benchmark, and integration conventions. Do not add another framework for one task.

Test public/domain/service/worker/CLI/component behavior through observable result/effect. Avoid private method/reflection/call-order tests that prevent safe refactoring.

Use backend framework-specific references for ASP.NET host/routes/auth/EF integration rather than duplicating those rules here.

### Cases And Assertions

Cover success, null/invalid/boundary, expected business failure, unexpected dependency failure, cancellation, repeated/concurrent operation, and cleanup changed by the task.

Use theories/parameterized cases for meaningful input partitions and keep case names/data readable. Avoid giant shared member data hiding which invariant failed.

Assert typed results/exceptions and stable public fields. Full exception/log/message string equality is brittle unless user-facing text is the contract.

### Async, Cancellation, And Background Work

Await every task and observe exceptions. Do not use `async void`, arbitrary delay, or fire-and-forget in tests.

Use controlled task completions, fake clocks, channels, barriers, cancellation sources, and deadlines to exercise ordering/cancellation/shutdown deterministically.

For workers/hosted services, start/stop through the public host/lifetime and assert pending work, scope disposal, errors, and no work after shutdown.

### Fixtures And Isolation

Use fixture scopes matching expensive resource lifetime and ensure parallel tests do not share mutable database/files/environment/clock/static/cache/container state accidentally.

Dispose hosts, scopes, clients, streams, temp directories/files, servers, timers, subscriptions, and cancellation sources. Restore culture/timezone/environment/current directory/global handlers.

Generate unique resource names and clean even after assertion/exception.

### Mocks, Fakes, And HTTP

Mock external boundaries (clock, filesystem, remote service, queue, mail, identity) with typed interfaces/handlers. Prefer stateful fakes when protocol/lifecycle matters.

For `HttpClient`, use a controlled `HttpMessageHandler` or repository test server; do not mock extension methods or create behavior unlike actual request/response disposal/cancellation.

Avoid mocking the service/domain/serializer/query under test or setting up every internal call.

### Blazor And UI

Use the repository component test framework for parameters, rendered states, forms, events, auth, lifecycle, and JS interop contracts. Keep real browser/hosting/render-mode evidence separate.

Assert semantic controls and stable action targets, not only markup snapshots. Dispose rendered components to prove subscriptions/interop cleanup.

### Property, Snapshot, And Mutation Tests

Property tests fit parsers, value objects, serialization, ordering, state machines, and algebraic invariants. Preserve minimal regression examples for failures.

Snapshots are appropriate for stable structured output with reviewed diffs; avoid huge volatile UI/log/exception snapshots.

Mutation testing can assess assertion sensitivity for critical pure logic but is not a universal coverage target.

### Runtime And Public API

Run tests under affected TFM/runtime/OS when behavior differs. For public packages, build a consumer and test nullability/analyzers/source generators/serialization/trim compatibility as applicable.

Code coverage percentage is supporting data, not proof; branch/invariant quality matters more than an arbitrary universal threshold.

## Verification Focus

- Run the narrow changed project/filter and ensure tests are discovered; expand only to affected solution lanes.
- Exercise deterministic cancellation/order/cleanup and isolate parallel global/resource state.
- Run actual integration/runtime/component layer when mocks cannot prove provider/hosting behavior.
- Verify release/published/trimmed behavior when build mode affects semantics.
- Record unavailable infrastructure precisely without claiming broader coverage.

## Evidence Focus

Name the public behavior/invariant, test layer, TFM/runtime/platform, and assertion. Test count, coverage percentage, mock expectations, or a green build without discovered tests is weak evidence.

## Unsafe Defaults

- C# testing loaded alongside duplicate ASP.NET Core testing guidance.
- New test framework or universal coverage target imposed on one task.
- Async void, fire-and-forget, arbitrary sleeps, or unobserved background errors.
- Shared mutable fixture state leaking across parallel tests.
- Internal call order asserted instead of public behavior.
- Component tests used to claim browser/render-mode/native integration.
