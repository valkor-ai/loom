# Rust Async Quality

## When To Use

- The task changes async functions, Tokio/async-std runtime usage, spawned tasks, channels, streams, async traits, shared async state, timeouts, cancellation, or graceful shutdown.
- Use this when correctness depends on runtime ownership, blocking boundaries, concurrency, or async error propagation.
- If the Rust code is synchronous and not in an async runtime path, do not introduce async.

## Implementation Focus

- Follow the existing async runtime. Do not mix Tokio, async-std, smol, or manual runtimes unless the repository already supports that boundary.
- Every spawned task needs lifecycle ownership, error handling, and shutdown behavior. Keep `JoinHandle` results observed when task failure matters.
- Do not block async executors with synchronous I/O, CPU-heavy work, or `std::thread::sleep`. Use async APIs or `spawn_blocking` for unavoidable blocking work.
- Use `tokio::join!` for independent infallible work, `try_join!` for all-or-first-error work, and channels/tasks when work must outlive one function scope.
- Use `select!` and cancellation channels/tokens for replaceable or long-running operations. Make cancellation branches clean up resources.
- Choose channel types by semantics: `mpsc` for queue work, `oneshot` for single replies, `watch` for latest state, `broadcast` for fan-out events. Bound channels unless unbounded behavior is justified.
- Avoid holding locks across `.await`, especially `std::sync` locks. Use async-aware locks sparingly and keep lock scope short.
- Prefer message passing over shared `Arc<Mutex<T>>` for task coordination when it simplifies ownership.
- Use timeouts for external I/O and long waits where callers need bounded behavior.
- Use `async-trait` only when trait-based async dispatch is needed and the allocation/object-safety tradeoff is acceptable.

## Decision Rules

- Select the runtime from repository facts and keep one runtime owner. A Tokio dependency in the baseline does not make every Rust task asynchronous.
- Use `join!` when all independent results are required, `try_join!` when the first error should cancel the group, and `select!` when one event wins and the other branches need cancellation cleanup.
- Give every spawned task an owner, a `JoinHandle`/error observation path, and a shutdown signal. Fire-and-forget work is acceptable only when the owner records failures and cancels it during teardown.
- Bound channels and fan-out. State queue capacity, backpressure, closed sender/receiver behavior, and whether ordering or partial success is part of the contract.
- Keep locks out of `.await` regions unless the lock and runtime explicitly support that boundary. Prefer message passing when it makes task ownership clearer.
- Use timeout and cancellation tokens around external I/O and long waits, and verify that all resources close when the cancellation branch wins.

## Verification Focus

- Run async tests using the repository runtime macro or test harness.
- Test success, timeout, cancellation, task failure, channel closure, and graceful shutdown paths touched by the task.
- Confirm no blocking call sits in an async hot path and no task is spawned without an owner.
- For streams/channels, test backpressure, closed sender/receiver, and dropped consumer behavior when relevant.

## Evidence Focus

- In the evidence summary, name the async decision: runtime selection, task lifecycle, blocking boundary, join/try_join, cancellation, channel type, lock scope, timeout, async trait, or shutdown proof.
