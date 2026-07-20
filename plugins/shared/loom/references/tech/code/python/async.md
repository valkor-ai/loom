# Python Async Quality

## When To Use

- The task changes `async`/`await`, asyncio tasks, TaskGroup usage, async clients, queues, streams, background jobs, async context managers, or sync/async boundaries.
- Use this when correctness depends on cancellation, timeouts, concurrent ordering, resource cleanup, or event loop safety.
- If the changed Python code is synchronous, do not convert it to async unless the task requires it or the surrounding framework is already async.

## Implementation Focus

- Keep async call chains async from the boundary inward. Do not call `asyncio.run` inside an already-running application loop or framework handler.
- Use structured concurrency where supported by the project's Python version, such as `asyncio.TaskGroup` for related tasks that should complete or fail together.
- Bound concurrent work with semaphores, queues, or task groups when input size can grow. Avoid unbounded task creation from user input, database rows, or remote lists.
- Propagate cancellation. Do not catch `CancelledError` and continue unless shutdown semantics explicitly require cleanup before re-raising.
- Apply timeouts around network calls, external services, queues, locks, and long-running operations where callers need a bounded response.
- Use async context managers for async clients, sessions, database transactions, locks, and resource lifecycles. Ensure `__aexit__` or `finally` closes resources.
- Keep blocking CPU or synchronous I/O out of the event loop. Use existing sync code in an executor only when replacing it is out of scope and the task owns the blocking boundary.
- Track background tasks in an owner that can cancel and await them during shutdown. Do not create fire-and-forget tasks without error handling.
- For async queues, define producer completion, consumer shutdown, `task_done`, and `join` behavior. Do not let consumers wait forever after production ends.
- Keep exception handling explicit for concurrent work: choose first-error fail-fast, partial success, or collected errors according to the business contract.

## Decision Rules

- Use `TaskGroup` when sibling tasks form one structured unit and should fail together; use separately owned tasks only when their lifecycle and error reporting are intentionally independent.
- Bound fan-out from inputs, rows, or remote pages with a semaphore or queue. State whether ordering is preserved, whether partial results are acceptable, and how producer completion wakes consumers.
- Put timeouts at the boundary that owns the latency contract. Distinguish timeout, cancellation, transport failure, and business rejection instead of converting all of them to a generic exception.
- Keep `asyncio.run` at a process/CLI boundary. Framework handlers and tests should use the existing event loop and runner rather than nesting event loops.
- Use async context managers for clients, sessions, locks, and transactions. A background task must have an owner, a done/error observation path, and shutdown cancellation.
- If synchronous work must remain, identify its executor/offload boundary and its capacity. Do not hide blocking database, filesystem, or CPU work in an `async def` body.

## Verification Focus

- Run async tests using the repository's configured pytest/asyncio plugin or framework test runner.
- Test success, exception, timeout, cancellation, and cleanup paths for changed async behavior.
- Add checks that blocking work is not executed on the event loop path when that risk is part of the change.
- Confirm background tasks, clients, queues, locks, and temporary resources are closed or cancelled after tests.
- Verify no pending tasks remain after the test runner completes and that cancellation is re-raised after cleanup.

## Evidence Focus

- In the evidence summary, name the async decision: structured concurrency, bounded concurrency, cancellation, timeout, async context manager, sync/async boundary, background task lifecycle, or queue shutdown.
