# Go Concurrency And Goroutine Ownership

## When To Use

Use this reference only when the task explicitly owns goroutines, channels, worker pools, pipelines, queues, timers, rate limits, shared state, async jobs, or shutdown behavior.

## Implementation Focus

### Lifecycle First

Every goroutine has a named owner, start point, stop/cancel signal, error/result path, and wait/join completion. Do not start hidden background work in constructors without returning a close/stop owner.

Do not launch a goroutine merely to make a blocking API appear async; preserve backpressure and cancellation at the real boundary.

Use `errgroup.WithContext` or the repository equivalent for sibling tasks that share cancellation/error propagation. Bound concurrency with `SetLimit`, semaphores, or worker pools according to workload/resources.

### Context And Shutdown

Select on `ctx.Done()` while sending, receiving, waiting, retrying, rate limiting, or doing timer work. A blocked channel operation must not prevent shutdown.

Define graceful shutdown order: stop intake, signal cancellation, drain or discard queued work by policy, stop producers, close outputs by owner, wait with deadline, release resources.

Preserve partial/committed work and idempotency across cancellation/process restart. Context cancellation does not roll back external effects automatically.

### Channel Ownership

The sending/producing owner closes a channel after all sends; receivers do not close shared inputs. Channels with multiple producers need one coordinator to close after producers finish.

Close channels to signal no more values, not as a general cancellation broadcast when context is the established boundary. Never send on/close a channel after close.

Choose buffer capacity as explicit bounded handoff/backpressure. A large buffer can hide slow consumers and increase memory/stale work; zero buffer enforces rendezvous.

Handle closed receive (`v, ok`/range), nil channels, abandoned consumers, and early pipeline errors so fan-in/fan-out goroutines cannot leak.

### Worker Pools And Pipelines

Define job identity, queue bound, enqueue behavior when full/closed, ordering/fairness, retry/idempotency, per-job timeout, panic/error handling, result ownership, and drain/cancel policy.

Close each pipeline stage's output only after all stage workers stop; use WaitGroups/coordinators. Propagate the first or all relevant errors without error channels that block because nobody receives.

Do not share loop variables or mutable request buffers across goroutines without per-iteration copies/ownership (including version-dependent range semantics).

### Mutexes, Atomics, And Once

Use a mutex for multi-field invariants and keep critical sections small without calling blocking/reentrant external code. Document which mutex protects which state.

`RWMutex` helps only under measured read-heavy contention; writer starvation/overhead may be worse. Never copy used mutex/atomic-containing structs.

Use `sync.Once`/`OnceValue` for one-time initialization with clear failure/panic semantics. An initialization error may need explicit state rather than Once.

Use `sync/atomic` for simple proven protocols/counters/flags with memory-model understanding. Atomic fields do not make compound invariants atomic.

### Timers, Tickers, And Retries

Stop tickers and timers when no longer owned. Reuse/reset timers with documented stop/drain semantics for the supported Go version; avoid `time.After` repeatedly in long loops when allocations/resources matter.

Retries need bounded attempts/time budget, context, backoff/jitter, failure classification, and idempotent operations. Do not retry permanent validation/auth failures or multiply retries across layers.

Rate limiters and semaphores release permits on every path and expose cancellation/overload behavior.

### Panic And Recovery

A panic in any goroutine can terminate the process. Recover only at deliberate process/job isolation boundaries, log safe context/stack, convert to failure, and preserve cleanup; do not use recover to ignore corrupted invariants.

### Race And Leak Safety

Maps and ordinary variables require synchronization for concurrent read/write. Channel transfer does not protect an object after both sides retain references.

Avoid goroutines blocked forever on sends, receives, locks, network calls without context, or wait groups. Capture goroutine profiles only to diagnose, not as routine product output.

## Verification Focus

- Test success, first-error, cancellation, timeout, full/closed queue, abandoned consumer, repeated stop, and shutdown with pending work.
- Use deterministic gates/channels/fake clocks rather than sleep-only ordering assertions.
- Run focused `go test -race` and repeated/stress cases for changed shared-state paths.
- Assert concurrency/queue/rate bounds and no work after stop.
- Check goroutine/resource cleanup with deadlines and diagnostics on failure.

## Evidence Focus

Name goroutine/channel owner, bound/backpressure, state synchronization, cancellation/shutdown policy, and deterministic/race proof. A WaitGroup or one clean run does not establish leak/race safety.

## Unsafe Defaults

- Goroutine started without stop/error/wait owner.
- Buffered channel used to hide blocking or unbounded work.
- Receiver/multiple senders closing a channel unsafely.
- Context ignored during blocked send/receive/timer/retry.
- Sleep-only concurrency tests.
- Atomic/RWMutex chosen without invariant or contention analysis.
- Retry layers multiplying non-idempotent effects.
