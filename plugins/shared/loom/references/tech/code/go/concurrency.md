# Go Concurrency Quality

Use this topic reference when `tech/code/go/concurrency.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes goroutines, channels, worker pools, queues, pipelines, timers, rate limiters, shutdown paths, shared mutable state, or async server/worker behavior.
- Use this when correctness depends on lifecycle ownership, cancellation, ordering, backpressure, or data race safety.
- If the changed code is purely synchronous Go, do not add goroutines or channels because this reference is available.

## Implementation Focus

- Every goroutine needs an owner, a stop condition, and an error/cancellation path. Do not start background goroutines from constructors unless the caller can stop them.
- Prefer `context.Context` for cancellation crossing API boundaries. Use done channels only for local ownership patterns where a context would be awkward or the repository already uses them.
- The sender side owns channel closing. Do not close a channel from a receiver, and do not close channels that may have multiple unsynchronized senders.
- Use buffered channels to model bounded queues or handoff capacity, not to hide blocked goroutines. If input can be unbounded, add concurrency limits or backpressure.
- Use `select` to respect cancellation while sending, receiving, waiting on timers, or acquiring rate limits. A blocked send inside a goroutine should not prevent shutdown.
- Prefer `errgroup` or a local equivalent when several goroutines need shared cancellation and first-error propagation. Avoid ad hoc error channels that can block because nobody drains them.
- Use `sync.Mutex`, `sync.RWMutex`, `sync.Once`, `atomic`, or channels according to ownership. Do not use channels only to protect simple shared state when a mutex is clearer.
- Stop tickers and timers. Drain or manage timer channels according to the standard library pattern when resetting timers.
- For worker pools and pipelines, define how input closes, how workers exit, how output closes, and how errors stop the pipeline. Do not leave fan-in goroutines waiting forever after cancellation.
- Add rate limiting or semaphores around expensive external calls, file work, CPU-heavy jobs, and user-triggered bulk operations when the input size can grow.

## Verification Focus

- Run `go test -race ./...` or a targeted race test when concurrency behavior changed.
- Add tests for cancellation, shutdown, first-error propagation, full/empty queue behavior, and closed input where relevant.
- Use timeouts in tests to catch leaked goroutines or blocked channels without making the suite flaky.
- For rate limiting or worker pools, test concurrency bounds or queue behavior with deterministic hooks rather than sleep-only assertions when possible.

## Evidence Notes

- Record `go.concurrency` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/go/concurrency.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the concurrency decision: goroutine lifecycle, cancellation, channel ownership, worker pool, pipeline, sync primitive, rate limit, or race proof.
