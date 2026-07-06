# JavaScript Async Quality

Use this topic reference when `tech/code/javascript/async.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes promises, `async`/`await`, fetch flows, timers, retries, cancellation, queues, streams, workers, background jobs, or event-driven JavaScript.
- Use this when correctness depends on ordering, parallelism, error propagation, cleanup, or backpressure.
- If the change is purely synchronous JavaScript, do not add async abstractions because this reference is available.

## Implementation Focus

- Choose parallelism intentionally. Use sequential `await` when later work depends on earlier output; use `Promise.all` only for independent all-or-nothing work; use `Promise.allSettled` when partial success is valid.
- For fetch or HTTP helpers, check response status before parsing success data. Preserve enough status/body information for callers to render a useful error or decide on retry.
- Add cancellation where user navigation, component unmount, request replacement, or long-running work can make the result stale. Wire `AbortController` through the actual call path rather than creating an unused controller.
- Pair timeouts with cancellation and timer cleanup. A timeout implemented with `Promise.race` should not leave the underlying operation running forever when the platform supports aborting it.
- Retry only idempotent or explicitly deduplicated operations. Do not retry form submissions, payments, writes, or state transitions unless the backend contract makes the retry safe.
- Limit concurrency for large batches, file processing, crawling, queue workers, or user-triggered bulk operations. Avoid unbounded `Promise.all(items.map(...))` on unbounded input.
- Do not leave floating promises. If fire-and-forget is intentional, attach error handling and make the lifecycle owner clear.
- Use `finally` or equivalent cleanup for locks, loading state, subscriptions, timers, temporary files, and active operation markers.
- For streams, prefer backpressure-aware APIs such as `pipeline` in Node or readable stream iteration where supported. Do not buffer large streams into memory unless the data size is bounded.

## Verification Focus

- Test success, failure, timeout, cancellation, and cleanup paths for changed async flows.
- Test ordering when operations must be sequential and independence when operations are intentionally parallel.
- Check that rejected promises are observed by tests or callers; no new unhandled rejection should appear in runtime output.
- For retries, verify retry count, backoff/stop condition, and non-retry behavior for unsafe or non-retryable errors.

## Evidence Notes

- Record `javascript.async` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/javascript/async.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the async decision: sequential flow, parallel batch, cancellation, timeout, retry policy, concurrency limit, stream handling, or cleanup guarantee.
