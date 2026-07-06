# PHP Async Quality

Use this topic reference when `tech/code/php/async.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. This file applies only to PHP async runtimes and async I/O boundaries.

## When To Use

- The task changes Swoole, ReactPHP, Amphp, Fiber-based scheduling, WebSocket/server loops, async queues, streams, timers, or concurrent HTTP/database clients.
- Use this when correctness depends on event-loop ownership, coroutine lifecycle, cancellation, timeout behavior, or avoiding blocking calls in async code.
- If the project is a regular synchronous PHP/Laravel/Symfony request-response app, do not introduce an async runtime just because this reference is available.

## Implementation Focus

- Follow the async stack already present in the repository. Do not mix Swoole, ReactPHP, Amphp, custom Fibers, and framework queue workers in one task without an explicit boundary.
- Keep exactly one clear owner for an event loop or long-running server process. Do not start loops inside controllers, request handlers, service constructors, or tests that cannot shut them down.
- Use async-compatible clients for network, database, Redis, filesystem, and timer work inside coroutine/event-loop paths. Move unavoidable blocking calls outside the hot path or behind an explicit worker/offload boundary.
- For Swoole, make coroutine boundaries visible: close clients, set timeouts, handle failed connects, and use wait groups/channels for fan-out work that must be joined before responding.
- For ReactPHP and Amphp, propagate promise/future failures instead of swallowing them in callbacks. Close sockets/connections on error and arrange timer/cancellation cleanup.
- Treat native Fibers as a primitive, not a scheduler. Do not build a bespoke async framework unless the repository already owns that abstraction.
- Use bounded channels, queues, or concurrency limits for fan-out. Unbounded producer/consumer paths need a task-owned reason and backpressure behavior.
- Keep shared mutable state out of callbacks where possible. When shared state is unavoidable, isolate mutation to one coroutine/task or use the runtime's concurrency-safe primitive.
- Long-running workers need graceful shutdown: signal handling, loop stop, in-flight request handling, connection close, and idempotent cleanup.

## Verification Focus

- Run the existing async/server smoke command when runtime code changes, and prove the process can start and stop cleanly.
- Test success, timeout, cancellation, connection failure, and handler exception branches touched by the task.
- For concurrent fan-out, verify all tasks are joined or cancelled and that partial failure returns the intended result.
- For server/request handlers, exercise at least one real request path and one invalid/error request path.
- Check that the changed async path does not contain obvious blocking calls such as `sleep`, synchronous HTTP clients, blocking PDO calls, or file I/O in an event-loop callback unless explicitly offloaded.

## Evidence Notes

- Record `php.async` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/php/async.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the async decision: runtime owner, coroutine boundary, event-loop cleanup, non-blocking client, timeout/cancellation, backpressure, or shutdown proof.
