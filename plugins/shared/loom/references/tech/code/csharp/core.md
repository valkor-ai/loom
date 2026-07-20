# C# Application And Library Delivery

## When To Use

Use this reference for task-owned C# application, domain, service, worker, CLI, library, or shared-contract code. Preserve target framework/language version, nullable/analyzer policy, async/error conventions, DI, serialization, and public API compatibility.

Version-specific language features, ASP.NET Core, EF Core, Blazor, performance, and testing are selected separately.

## Implementation Focus

### Nullability And Invariants

Treat nullable reference annotations as a public correctness contract. Validate external input, narrow nullable values with control flow, and model optional/required states explicitly.

Do not use `!`, `#nullable disable`, broad warning suppression, or default-initialized required members to silence a real initialization/validation gap. A justified interop/framework assertion stays local and documented by the invariant.

Keep DTO, domain, persistence, configuration, and UI models separate when required/nullability/serialization/lifetime differ.

### Async And Cancellation

Keep I/O paths asynchronous end to end. Avoid `.Result`, `.Wait()`, `GetAwaiter().GetResult()`, sync wrappers, and unobserved fire-and-forget work.

Accept/forward `CancellationToken` when the owner or framework can cancel. Cancellation is not failure: preserve `OperationCanceledException`, stop starting new work, and clean up/rollback owned resources.

Every background task needs a host/lifetime owner, error observation, stop policy, and shutdown deadline. `async void` is limited to required event-handler signatures with local error handling.

Use `Task.WhenAll` only when operations are independent and concurrency/resource limits permit it. Preserve all failures and cancellation semantics rather than awaiting tasks repeatedly in a way that hides exceptions.

### Resource And Disposal Ownership

Use `using`/`await using`, `IDisposable`/`IAsyncDisposable`, and DI ownership consistently for streams, responses, DB contexts, timers, subscriptions, locks, native handles, channels, and scopes.

Do not dispose container-owned services manually or capture scoped/disposable services in singletons/static state. A factory-created scope/service is disposed by its explicit owner.

Return streams/enumerables only when the caller knows who owns the underlying resource and how long enumeration remains valid.

### Dependency Injection And Lifetimes

Constructor injection makes required dependencies explicit. Keep service lifetimes compatible: singleton cannot capture scoped/transient-disposable state; workers create scopes per unit of work when required.

Avoid service locator calls and broad `IServiceProvider` injection except in composition/factory boundaries. Resolve keyed/named strategies through established typed factories when dynamic selection is product behavior.

Keep pure domain logic free of framework container/config/logging dependencies where repository architecture separates it.

### Errors And Results

Follow the repository's expected-failure model: result/discriminated union, validation result, typed exception, or boundary-specific error. Do not add a homegrown `Result<T>` when a standard exists.

Exceptions represent exceptional failures and preserve inner exception/stack; expected business rejection remains typed/actionable. Catch only when translating, adding safe context, compensating, or retrying with policy.

Do not log and rethrow the same exception at every layer or expose raw provider messages, stack traces, secrets, or sensitive payloads.

### Values, Collections, And Enumeration

Use records/read-only/init types when value semantics and lifecycle fit; do not choose records for mutable identity-rich entities merely for brevity.

Be explicit about `IEnumerable<T>` laziness, multiple enumeration, disposal, mutation during enumeration, and materialization bounds. Return `IReadOnlyList<T>`/immutable collections only when the contract truly prevents or isolates mutation.

Use `DateTimeOffset`/UTC, decimal, culture-aware parsing/formatting, and checked numeric conversion according to domain/wire/storage requirements.

### Configuration And Logging

Bind strongly typed options at composition boundaries and validate required values before work starts. Avoid scattered string keys and insecure/local production fallbacks.

Use structured logs with stable event context and no credentials/tokens/personal/sensitive payloads. Avoid logging the same error repeatedly across layers.

### Serialization And Public API

Keep wire/storage serializers explicit about names, null/default/unknown fields, enums, dates, decimals, polymorphism, and backward compatibility. Static types do not validate untrusted payloads automatically.

For public assemblies, preserve accessibility, signatures, generic constraints, nullability, exceptions, attributes, serialization, and binary/source compatibility within accepted versioning policy. Document public behavior where repository policy requires it.

## Verification Focus

- Run focused build/analyzers and tests for changed projects under the actual target framework/language version.
- Treat new nullable/analyzer warnings in changed code as defects unless locally justified.
- Exercise invalid/null/boundary input, expected and unexpected errors, cancellation, disposal, and DI lifetime behavior.
- Test serialization/configuration/public API compatibility when those boundaries change.
- Verify no blocking async calls, unowned background work, or sensitive logging was introduced.

## Evidence Focus

Name the nullability/async/cancellation owner, DI/resource lifetime, error model, serialization/config/public API decision, and assertion proving behavior. A warning-free build alone does not prove cancellation, disposal, or runtime contract safety.

## Unsafe Defaults

- Null-forgiving/suppression used instead of initialization or validation.
- Sync-over-async or unobserved fire-and-forget work.
- Scoped/disposable dependency captured by singleton/static state.
- New generic Result/error abstraction duplicating repository conventions.
- Lazy enumerable escaping disposed resources or enumerated repeatedly unknowingly.
- Raw exceptions or sensitive values logged/serialized publicly.
