# Spring Boot Asynchronous Processing

Use Spring asynchronous execution only when the task owns a real asynchronous boundary. `@Async` changes execution and failure semantics; it is not a performance annotation for making a blocking request appear non-blocking.

## Boundary Choice

Choose the mechanism that matches durability and coupling:

| Need | Suitable Boundary |
|---|---|
| Short, non-durable work tied to one application instance | Managed `TaskExecutor` and `@Async` |
| Durable work that must survive restart | Queue, job store, or persisted work record |
| Scheduled reconciliation | Spring scheduler with explicit overlap/idempotency policy |
| Reactive I/O composition | Reactor pipeline, not `@Async` around reactive code |

Do not use an in-memory executor for business work that the accepted contract says must not be lost.

## Proxy And Invocation

`@Async` is applied by a Spring proxy; same-class self-invocation, private methods, and objects created with `new` do not cross the asynchronous proxy.

```java
@Service
final class NotificationDispatcher {
    private final NotificationSender sender;

    NotificationDispatcher(NotificationSender sender) {
        this.sender = sender;
    }

    @Async("notificationExecutor")
    public CompletableFuture<DispatchResult> dispatch(NotificationCommand command) {
        return CompletableFuture.completedFuture(sender.send(command));
    }
}
```

Keep the async entry point in a separate bean when a synchronous service triggers it. Make the return type reflect whether callers can observe completion: `CompletableFuture<T>` for observable completion, `void` only for explicitly fire-and-forget work with an exception handler and independent failure signal.

## Executor Ownership

Define a named executor for owned workloads. Configure:

- core/max concurrency based on the workload type
- bounded queue capacity
- thread naming
- rejection behavior
- task decoration for supported context propagation
- graceful shutdown and await timeout

An unbounded queue hides overload until memory pressure. `CallerRunsPolicy` changes latency and execution context; use it only when backpressure on the caller is acceptable. Dropping work requires an explicit observable failure and retry/recovery path.

Do not copy security, request, locale, or logging ThreadLocals manually. Use supported task decorators/context propagation and propagate only data required by the work. Never rely on request-scoped beans after the request completes.

## Transactions And Side Effects

The async method runs in another thread and does not inherit the caller's transaction. Passing a managed JPA entity across the boundary risks detached state and stale data. Pass immutable identifiers/commands and load required state inside the async operation.

Trigger async work after commit when it depends on committed data. Use transaction synchronization, domain events with an after-commit listener, or a durable outbox according to reliability requirements. An after-commit in-memory callback is still lost on process failure.

Define idempotency for retries or duplicate dispatch. Record status/progress when users or operators need to observe completion.

## Failure And Cancellation

- Complete returned futures exceptionally; do not convert failures to silent success.
- Configure `AsyncUncaughtExceptionHandler` only for `void` methods.
- Distinguish retryable dependency failure from invalid business input.
- Preserve interrupt/cancellation signals where the underlying work supports them.
- Stop accepting new tasks during shutdown and bound the wait for in-flight work.

## Verification Focus

Useful async evidence includes:

- proof that invocation crosses the Spring proxy and executor thread
- bounded queue/rejection behavior
- success and exceptional completion
- after-commit ordering for database-backed work
- duplicate-effect protection
- context/correlation propagation without request-scope leakage
- shutdown behavior for queued and running work

Use futures, latches, Awaitility, virtual time, or completion records. Do not assert asynchronous behavior through arbitrary sleeps.

## Unsafe Defaults

- Adding `@Async` to a same-class method call.
- Using the common pool or an unbounded executor by accident.
- Passing JPA entities into another thread.
- Assuming the caller transaction crosses the async boundary.
- Fire-and-forget work with no failure visibility.
- Using in-memory async execution for required durable work.
