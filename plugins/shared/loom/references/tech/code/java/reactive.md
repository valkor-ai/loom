# Java Reactive WebFlux Quality

This file applies to WebFlux, Reactor, R2DBC, WebClient, and reactive endpoint changes.

## When To Use

- The task changes `Mono`, `Flux`, WebFlux controllers, reactive services, R2DBC repositories, `WebClient`, streaming responses, or async/reactive error handling.
- Do not convert a blocking Spring MVC/JPA feature into WebFlux because this reference is available.
- If a reactive path must call blocking code, isolate it deliberately and explain the scheduler/boundary; do not hide blocking calls inside `map` or `flatMap`.

## Implementation Focus

- Keep reactive chains non-blocking end to end. Do not call `.block()`, `.subscribe()` for side effects, blocking repositories, or synchronous HTTP clients inside request handling.
- Controllers should return `Mono`/`Flux` and leave business composition to services. Services should compose repository/client calls and map errors, not force execution.
- Use `switchIfEmpty` for not-found paths and `onErrorMap`/`onErrorResume` for explicit business or client errors. Do not swallow errors into empty success responses.
- Pick operators based on behavior: `map` for synchronous transformation, `flatMap` for async composition, `concatMap` when ordering matters, `zip` for independent values, and `then` when only completion matters.
- Use R2DBC repositories for reactive persistence. Mixing JPA repositories in reactive endpoints creates blocking behavior unless isolated behind a bounded scheduler and justified.
- For `WebClient`, set timeouts/retries only where the business operation can safely retry. Do not retry non-idempotent writes unless the API contract supports it.
- Keep backpressure and response size in mind. Unbounded `Flux` list endpoints need pagination, limit, streaming contract, or explicit bounded data reason.
- For transactions, use reactive transaction support where available. Do not assume imperative `@Transactional` has the same behavior in every reactive chain.
- Ensure side effects are inside the reactive pipeline and are testable; avoid manual subscription in application code.
- Preserve thread context only through supported mechanisms. Do not rely on ThreadLocal request/security context without reactive support.

## Verification Focus

- Use `StepVerifier` for reactive service and repository behavior, including success, empty/not-found, and error paths.
- For WebFlux controllers, use `WebTestClient` or the repository's equivalent to prove status codes and response body.
- Test timeout/retry/cancellation behavior when the task adds external calls or long-running reactive flows.
- Run build/test and ensure no blocking API is introduced in reactive request paths. If BlockHound or similar tooling exists, use it.
- For R2DBC changes, test against the configured reactive database driver or repository slice.

## Evidence Focus

- In the evidence summary, name the reactive chain or endpoint and the success/empty/error/cancellation behavior verified.
