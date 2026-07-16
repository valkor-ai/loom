# JavaScript Async Quality

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

### Combinator And Failure Selection

Choose the combinator from the business outcome:

| Outcome | Pattern | Failure meaning |
|---|---|---|
| All independent results are required | `Promise.all` | first rejection fails the operation; define cleanup for other work |
| Each result is useful independently | `Promise.allSettled` | return or record per-item success and failure |
| First successful source is acceptable | `Promise.any` | reject only after every candidate fails; preserve `AggregateError` context |
| First settlement ends the race | `Promise.race` | the losing operation still needs cancellation when it can continue running |
| Ordered dependency | sequential `await` or `for await` | later work must not start before prior state is valid |

Do not use `Promise.all` as an unbounded batch scheduler. For large or user-controlled collections, add a bounded queue, define whether failures stop or continue the batch, and preserve input-to-result correlation.

### Timeout And Cancellation Ownership

Timeout, abort, and retry are separate decisions. A timeout must stop or detach the underlying operation, clear its timer, and return an error category callers can distinguish from validation or business rejection. Pass one `AbortSignal` through the complete call chain; do not create a controller in a helper that no owner can abort.

Retry only transient failures and keep the attempt budget outside the operation's business result. Backoff must be cancellable. A retry around a non-idempotent write requires an idempotency key or an equivalent deduplication contract.

### Queue And Stream Boundaries

An async generator should define page/cursor advancement, termination, duplicate handling, and cleanup when iteration stops early. A concurrency queue should release waiters in a `finally` path and define behavior when queued work is cancelled. For Node streams, use `pipeline` or equivalent propagation so source, transform, destination, and abort all share failure and cleanup semantics.

## Verification Focus

- Test success, failure, timeout, cancellation, and cleanup paths for changed async flows.
- Test ordering when operations must be sequential and independence when operations are intentionally parallel.
- Check that rejected promises are observed by tests or callers; no new unhandled rejection should appear in runtime output.
- For retries, verify retry count, backoff/stop condition, and non-retry behavior for unsafe or non-retryable errors.
- For combinators and queues, verify partial-failure policy, bounded concurrency, input/result correlation, early iterator termination, and cleanup of losing or cancelled work.

## Evidence Focus

- In the evidence summary, name the async decision: sequential flow, parallel batch, cancellation, timeout, retry policy, concurrency limit, stream handling, or cleanup guarantee.
