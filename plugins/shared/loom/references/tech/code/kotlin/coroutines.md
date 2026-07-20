# Kotlin Coroutines Quality

## When To Use

- The task changes suspend functions, coroutine scopes, dispatchers, Flow, StateFlow, SharedFlow, channels, background work, cancellation, retries, or async integration.
- Use this when correctness depends on structured concurrency, lifecycle ownership, backpressure, or async error propagation.
- If changed Kotlin code is synchronous and not in an async framework path, do not introduce coroutines because this reference is available.

## Implementation Focus

- Use structured concurrency. Do not use `GlobalScope` in production code; work should be owned by a request, service, ViewModel, application, or explicitly managed scope.
- Choose dispatchers by work type: CPU on `Default`, blocking I/O on `IO`, UI state updates on main/lifecycle scopes. Do not hide blocking calls inside default suspend functions.
- Preserve cancellation. Re-throw `CancellationException`; do not catch `Exception` and convert cancellation into a normal failure state.
- Use `coroutineScope` when all children must succeed, and `supervisorScope` when independent children can fail without cancelling siblings. Make this failure policy visible.
- Use `async` only when a result is awaited and parallelism is beneficial. Use `launch` for lifecycle-owned work that does not return a value, with explicit error handling.
- For Flow, distinguish cold data streams, `StateFlow` current state, and `SharedFlow` events. Do not use `StateFlow` for one-shot events or `SharedFlow` for state that needs an initial value.
- Use Flow operators deliberately: `debounce` for user input, `distinctUntilChanged` to suppress duplicates, `flatMapLatest` for replaceable requests, `buffer` or `conflate` only when dropping/decoupling is acceptable.
- Bound producer-consumer work with channels, buffers, or semaphores when input can grow. Do not launch unbounded async work from lists or streams.
- Own background scope cancellation during shutdown, component disposal, or ViewModel clearing. Do not leak jobs beyond their lifecycle.
- Keep retries limited to safe/idempotent operations and document retryable exception types.

## Decision Rules

- Put the scope at the owner that can cancel the work: request scope for request work, a ViewModel or screen scope for UI work, and an application-owned scope only for work that must survive individual requests or screens.
- Keep dispatcher selection at the blocking boundary. A repository that calls a blocking driver should isolate that call; callers should not guess which dispatcher a generic `suspend` function requires.
- Use `StateFlow` for observable state with a current value, `SharedFlow` for events or broadcasts, and a cold `Flow` when each collector owns collection. Document replay and buffer policy when events can be lost.
- Choose `flatMapLatest`, `buffer`, or `conflate` only after stating which work or values may be cancelled, delayed, or dropped. Do not hide this policy in a helper with a generic name.
- Retry only idempotent operations or operations protected by an idempotency key. Bound attempts and delay, and surface the terminal error through the owning state or result type.

## Verification Focus

- Use `runTest` or the repository coroutine test setup for coroutine code.
- Use Turbine or equivalent for Flow emission order, completion, cancellation, and error assertions when Flow behavior changed.
- Test cancellation, timeout, first-error/partial-success policy, dispatcher-sensitive boundaries, and lifecycle cleanup where touched.
- Confirm no `GlobalScope`, `runBlocking` in production paths, swallowed cancellation, or unbounded parallelism was introduced.
- For lifecycle changes, prove that the parent scope is cancelled and that child jobs do not outlive the request, screen, service, or shutdown owner.

## Evidence Focus

- In the evidence summary, name the coroutine decision: scope ownership, dispatcher, cancellation, supervisor policy, Flow type, buffering/backpressure, retry policy, or lifecycle cleanup.
