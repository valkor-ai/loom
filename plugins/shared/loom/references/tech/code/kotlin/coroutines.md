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

## Verification Focus

- Use `runTest` or the repository coroutine test setup for coroutine code.
- Use Turbine or equivalent for Flow emission order, completion, cancellation, and error assertions when Flow behavior changed.
- Test cancellation, timeout, first-error/partial-success policy, dispatcher-sensitive boundaries, and lifecycle cleanup where touched.
- Confirm no `GlobalScope`, `runBlocking` in production paths, swallowed cancellation, or unbounded parallelism was introduced.

## Evidence Focus

- In the evidence summary, name the coroutine decision: scope ownership, dispatcher, cancellation, supervisor policy, Flow type, buffering/backpressure, retry policy, or lifecycle cleanup.
