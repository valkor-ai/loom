# Java Reactor And Reactive Data Fundamentals

This reference owns Reactor composition, non-blocking boundaries, backpressure, cancellation, reactive context, and R2DBC transaction semantics. Spring WebFlux controllers, WebClient configuration, security filters, and web tests belong to Spring Boot references.

## When To Use

Use this reference when Java implementation work owns Reactor pipelines, reactive streams, R2DBC transactions, cancellation/resource cleanup, backpressure, scheduler boundaries, or reactive context propagation. A Spring Boot API task selects it only when the accepted stack includes WebFlux/reactive technology or the task explicitly owns reactive implementation.

Do not use it for ordinary Spring MVC, JPA, synchronous HTTP clients, or merely asynchronous business wording. Framework transport and client configuration remain in the selected Spring Boot references.

## Implementation Focus

### Non-Blocking Boundary

Keep reactive chains non-blocking end to end. Do not call `.block()`, synchronous HTTP clients, JPA repositories, filesystem APIs, or blocking SDKs on event-loop threads.

When an unavoidable blocking adapter exists, isolate it on a bounded scheduler at one explicit boundary and account for concurrency, queueing, cancellation, and shutdown. Do not scatter `subscribeOn` as a general repair.

### Operator Semantics

- `map`: synchronous transformation
- `flatMap`: asynchronous composition without ordering guarantee
- `concatMap`: ordered asynchronous composition
- `flatMapSequential`: concurrent work with ordered results
- `zip`: combine independent publishers
- `switchIfEmpty`: explicit absence/not-found branch
- `then`: completion when prior values are intentionally discarded

Avoid nested subscriptions. Application code should return publishers to the owning framework. Manual `subscribe()` hides lifecycle, cancellation, and failure.

### Error Semantics

Use `onErrorMap` to translate errors and `onErrorResume` only for an accepted fallback. Do not turn failures into empty successful publishers. Place retry at the operation boundary and classify safe failures; never retry non-idempotent writes without a deduplication contract.

Use `doOn...` for observation, not business mutation. Cleanup belongs in `usingWhen`, `doFinally`, or resource-specific operators with success/error/cancel behavior defined.

### Backpressure And Bounds

Unbounded `Flux` results need pagination, streaming, rate limits, or a proven bounded source. Define buffering and overflow behavior. Avoid collecting arbitrarily large streams into memory.

Choose concurrency and prefetch deliberately for fan-out work. More concurrency can overload downstream services and break ordering assumptions.

### Context And Threading

Reactive execution can move between threads. Do not rely on ThreadLocal request, transaction, security, or logging state without supported context propagation. Put immutable correlation/security data in Reactor Context through framework-supported integrations.

### R2DBC And Transactions

Use reactive repositories/drivers for reactive persistence. Reactive transactions must wrap subscription through supported operators or transactional proxies; imperative transaction assumptions do not automatically apply.

Do not pass mutable entities across concurrent operators. Define write ordering and conflict behavior explicitly.

### Cancellation

Cancellation is a normal terminal signal. Ensure resources close and avoid side effects that continue invisibly after the caller cancels unless the operation is intentionally durable and decoupled.

## Verification Focus

Use `StepVerifier` and virtual time for:

- success, empty, and error branches
- ordering and concurrency
- timeout/retry classification
- backpressure/bounded collection behavior
- cancellation and cleanup
- reactive transaction commit/rollback
- absence of blocking calls when detection tooling exists

## Evidence Focus

Use `StepVerifier` or an equivalent subscriber-based assertion for publisher signals. Evidence should identify the success, empty, failure, cancellation, ordering, or retry path and assert terminal signals and side effects, not only emitted values.

For non-blocking claims, include a boundary-specific check or detector result when available. For R2DBC, retries, and resource cleanup, use integration evidence that exercises subscription, transaction/resource lifecycle, and the selected provider or adapter.

## Unsafe Defaults

- `.block()` in a reactive request path.
- Manual `subscribe()` for business side effects.
- Hiding blocking work inside `map`/`flatMap`.
- `onErrorResume` that returns empty/fake success.
- Unbounded `collectList`, buffers, or fan-out concurrency.
- Assuming ThreadLocal or imperative transaction state propagates.
