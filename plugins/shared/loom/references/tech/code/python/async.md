# Python Async Quality

Use this topic reference when `tech/code/python/async.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

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

## Verification Focus

- Run async tests using the repository's configured pytest/asyncio plugin or framework test runner.
- Test success, exception, timeout, cancellation, and cleanup paths for changed async behavior.
- Add checks that blocking work is not executed on the event loop path when that risk is part of the change.
- Confirm background tasks, clients, queues, locks, and temporary resources are closed or cancelled after tests.

## Evidence Notes

- Record `python.async` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/python/async.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the async decision: structured concurrency, bounded concurrency, cancellation, timeout, async context manager, sync/async boundary, background task lifecycle, or queue shutdown.
