# Angular Reactive Client Flows With RxJS

Use RxJS guidance for task-owned stream semantics, API bindings, cancellation, ordering, fan-out, subscription lifecycle, or shared observable results. Do not turn straightforward local signal state into streams by convention.

## Choose The Correct Reactive Primitive

Use signals for synchronous local/derived view state and Observables for asynchronous sequences, cancellation, multiple emissions, router/events, HTTP composition, and NgRx effects. Convert at the UI boundary with `toSignal`/`toObservable` when it clarifies ownership; avoid repeated conversion loops.

Subjects are event sources, not generic mutable stores. Expose `asObservable()` when a subject is necessary and keep writes private. Prefer `BehaviorSubject` only when every subscriber needs a current value and the initial value is meaningful.

## Flattening Semantics

Select higher-order operators from business concurrency:

| Requirement | Operator |
|---|---|
| Latest search/filter/detail wins | `switchMap` |
| Independent bounded operations may overlap | `mergeMap` with concurrency |
| Writes must preserve order | `concatMap` |
| Ignore duplicate submit while active | `exhaustMap` |

```typescript
readonly results$ = this.query.valueChanges.pipe(
  debounceTime(250),
  map(value => value.trim()),
  distinctUntilChanged(),
  switchMap(query => this.search.search(query).pipe(
    map(items => ({ kind: 'ready' as const, items })),
    catchError(error => of({ kind: 'error' as const, error: mapApiError(error) })),
  )),
);
```

Do not use `switchMap` for writes that must complete or `mergeMap` for double-click-sensitive commands. Operator choice is part of product correctness.

## Loading, Error, And Finalization

Place `catchError` inside the boundary that should recover. Catching outside a long-lived action/search stream can terminate it permanently. Do not convert every failure to `[]`/`null`; preserve typed validation, conflict, permission, unavailable, and transport states.

Use `finalize` for loading cleanup that must run on success, error, and cancellation, but distinguish cancellation from a visible failure when UX behavior differs.

Retry only accepted transient/idempotent operations with bounded attempts, delay/jitter, cancellation, and final error. Never retry validation/auth/business conflict or non-idempotent writes by generic interceptor/operator.

## Combination And Completion

Use `combineLatest` for long-lived latest-value inputs, `forkJoin` for finite operations that must all complete, `zip` for positional pairs, and `merge` for independent emissions. Ensure each source has the required initial/completion behavior; a `combineLatest` source that never emits can stall the view.

Model partial failure explicitly when one source may fail without invalidating the entire surface. Avoid nested subscriptions for dependent calls; compose through operators so cancellation and errors remain visible.

## Lifecycle And Teardown

Prefer async pipe, `toSignal`, or `takeUntilDestroyed` for component/route lifetimes. Capture `DestroyRef` when calling outside an injection context.

Services with application lifetime should not use component destruction as a cleanup model. Define cache/subscription lifetime explicitly and release WebSocket/event/browser resources when the owning provider ends.

Avoid subscriptions inside subscriptions, forgotten event streams, and imperative subscription arrays. Never subscribe only to trigger an HTTP request while discarding its error/cancellation semantics.

## Sharing And Caching

Use `shareReplay({ bufferSize: 1, refCount: true })` only when sharing one result is intentional and reset/invalidation semantics are understood. Process-wide replay can leak user/tenant-specific data or keep stale results after mutation/login changes.

For server data, define source of truth, freshness, invalidation, error retention, and refetch behavior. RxJS sharing is not automatically a durable cache or state-management architecture.

## Backpressure And Event Volume

Bound typeahead, resize, scroll, upload, and polling event rates with suitable debounce/throttle/sample/buffer behavior. Keep polling visibility, cancellation, overlap, and retry explicit; stop polling when the surface/identity no longer owns it.

Limit `mergeMap` concurrency for file/batch/network work. Unbounded parallel requests can exhaust browser/provider resources and scramble user feedback.

## Verification

- Use focused scheduler/marble tests when timing, cancellation, order, or retry is the claimed behavior.
- Prove latest-wins, duplicate-submit prevention, ordered writes, bounded concurrency, and partial failure where owned.
- Verify loading/disabled state after success, failure, cancellation, and rapid repeated actions.
- Test teardown on component destroy, route change, modal close, identity change, and stream error.
- Verify shared result invalidation and no cross-user/tenant leakage.
- Exercise the real HTTP adapter mapping when status/error semantics drive the stream.

## Delivery Evidence

Name the source stream, chosen concurrency/recovery rule, and emission/subscription assertion proving it. A stream type or one successful emission cannot prove cancellation, teardown, retry bounds, ordering, or cache invalidation.

## Unsafe Defaults

- Prose keywords selecting RxJS guidance without a reactive/API-binding task.
- Subjects used as unstructured global mutable state.
- `switchMap` applied to required writes or `mergeMap` to duplicate submissions.
- Errors converted to empty data and long-lived streams terminated accidentally.
- Unbounded retry, polling, replay, or parallelism.
- Nested subscriptions and missing teardown.
- Shared replay retaining identity-sensitive stale data.
