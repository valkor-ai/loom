# Angular RxJS Quality

This file applies RxJS discipline to task-owned Angular services, components, facades, guards, resolvers, effects, and async UI workflows.

## When To Use

- The task creates or changes observables, subscriptions, HTTP streams, event streams, search/typeahead, polling, retries, cancellation, guards, resolvers, or effects.
- Use this when stream semantics determine correctness: latest-wins, sequential processing, duplicate-submit prevention, concurrent fan-out, caching, or teardown.
- Keep component rendering rules in the Angular component reference and NgRx store contracts in the NgRx reference.

## Implementation Focus

- Pick higher-order operators by business semantics. Use `switchMap` for latest-wins search/filter/detail reloads, `concatMap` for ordered queues, `mergeMap` for independent parallel work, and `exhaustMap` for duplicate-submit prevention.
- Handle errors inside the correct stream boundary. A recoverable list load may return an empty/error view model; a mutation should surface a typed failure state rather than silently completing.
- Use `takeUntilDestroyed()` or existing repository teardown helpers for component/service subscriptions that have a lifecycle. Prefer `async` pipe or signals where that is already the app style.
- Avoid nested subscriptions. Compose streams with operators so loading, success, error, cancellation, and cleanup remain testable.
- Use `debounceTime` and `distinctUntilChanged` for typeahead or live filters that hit APIs. Keep validation and minimum-search-length rules explicit before network calls.
- Use `shareReplay({ bufferSize: 1, refCount: true })` only when sharing cached read data is intentional. Avoid unbounded replay or stale process-wide caches for user-specific or permission-sensitive data.
- Convert streams to signals only at UI boundaries where signal rendering improves clarity. Keep long-lived domain streams and effects observable-based if the repository already models them that way.
- Keep loading state accurate on error and cancellation. Avoid setting loading false only in success branches.
- Do not log errors as a substitute for user-visible business feedback or task evidence.

## Verification Focus

- Test operator behavior for cancellation, duplicate submits, sequential ordering, parallel completion, and retry/error paths where those semantics are task-owned.
- Verify subscriptions clean up on component destroy, route change, modal close, repeated filter changes, and failed API calls.
- Verify loading state and disabled controls return to the correct state after success, failure, cancellation, and rapid repeated actions.
- For cached streams, verify cache invalidation after mutation or user/context changes.
- Use marble tests or focused observable tests when timing/order is the risk; use component/service tests when UI-visible state is the risk.

## Evidence Focus

- In the evidence summary, name the RxJS decision: latest-wins, duplicate-submit prevention, sequential queue, parallel fan-out, lifecycle teardown, cache sharing, retry policy, or typed error recovery.
