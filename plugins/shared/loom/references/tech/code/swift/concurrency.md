# Swift Concurrency Quality

This file applies to Swift async/await and concurrency isolation.

## When To Use

- The task changes async functions, actors, `@MainActor` code, task groups, `async let`, `Task`, cancellation, `AsyncSequence`, continuations, Sendable types, or concurrency-related UI/server behavior.
- Use this when correctness depends on structured concurrency, thread safety, cancellation, or crossing actor/executor boundaries.
- If the touched Swift code is synchronous and has no concurrency boundary, do not introduce async.

## Implementation Focus

- Prefer native async APIs and structured concurrency. Do not wrap an async API in a continuation just because older callback examples exist.
- Use actors for mutable shared state that crosses tasks. Keep synchronous locks and `@unchecked Sendable` as last-resort boundaries with a clear invariant.
- Mark UI-facing view models and UI update methods with `@MainActor` when they mutate UI state. Keep non-UI work outside the main actor when it can run independently.
- Use `async let` for a small fixed set of independent operations and task groups for dynamic fan-out. Preserve result ordering explicitly when callers depend on order.
- Every long-running `Task` needs ownership, cancellation, and cleanup. Avoid detached tasks unless breaking structured concurrency is intentional.
- Check cancellation in loops, streams, and long waits. Propagate `CancellationError` or translate it into the repository's expected user/system behavior.
- For `AsyncSequence`, define termination and resource cleanup with `onTermination` or equivalent lifecycle handling.
- Continuations must resume exactly once on every success, failure, cancellation, and early-return branch. Prefer checked continuations unless the repository has a measured reason not to.
- Make cross-task data `Sendable` when safe, and fix compiler warnings rather than hiding them.

## Verification Focus

- Run `swift build`/target build and the repository's async tests.
- Test success, failure, cancellation, timeout or stream termination, actor-isolated mutation, and main-actor UI update paths touched by the task.
- Compile with concurrency warnings or warnings-as-errors when the project enables them.
- For task groups or fan-out, verify partial failure and ordering behavior.

## Evidence Focus

- In the evidence summary, name the concurrency decision: actor isolation, MainActor boundary, async let/task group, task ownership, cancellation, AsyncSequence cleanup, continuation safety, or Sendable proof.
