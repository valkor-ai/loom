# C++ Concurrency And Async Ownership

## When To Use

Use this reference only when the task explicitly owns threads, executors/pools, synchronization, atomics, concurrent queues, futures, parallel algorithms, coroutines, cancellation, or shared mutable state.

## Implementation Focus

### Ownership And Shutdown

Every thread/task/pool/coroutine has an owner, start condition, cancellation/stop signal, error channel, join/drain policy, and shutdown deadline.

Prefer `std::jthread`/stop tokens when the accepted standard and repository support them. Do not detach threads unless work/resources intentionally have process lifetime and failure is observable.

Define whether shutdown drains queued work, cancels pending work, rejects new work, or persists/requeues it. Destructors must not deadlock or silently abandon required effects.

### Mutexes And Invariants

Protect invariants, not individual fields. Use RAII guards and the narrowest lock duration that preserves atomic state transitions.

Do not hold locks across user callbacks, blocking I/O, future waits, coroutine suspension, logging that can reenter, or long computation unless explicitly safe.

Use consistent lock ordering or `std::scoped_lock` for multiple mutexes. Avoid recursive mutexes as a fix for unclear ownership.

Reader-writer locks help only with measured read-heavy contention and can starve; benchmark against a normal mutex.

### Condition Variables And Queues

Wait in a predicate loop under the associated mutex because wakeups are spurious and state can change before reacquisition. Update shared state before notification according to the invariant.

Bound queues/pools where producers can outrun consumers. Define full behavior (block, timeout, reject, drop/coalesce), priority/fairness, exception propagation, and shutdown wakeup.

Tests and production waits need deadlines/cancellation where indefinite blocking is unsafe.

### Atomics And Memory Ordering

Use atomics for independent state or a proven lock-free protocol. Start with sequential consistency unless a documented happens-before proof justifies weaker ordering.

Relaxed order provides atomicity only. Release/acquire synchronizes only through matching operations; fences and mixed atomic/non-atomic access require precise reasoning.

Compare-exchange updates the expected value on failure and may spuriously fail for weak variants. Account for ABA, reclamation, wraparound, and lifetime before using lock-free structures.

Do not use `volatile` for thread synchronization.

### Futures, Exceptions, And Cancellation

Specify `std::async` launch policy; default policy may defer work. Retrieve futures or otherwise handle exceptions/results so failures are not silently discarded.

Promises must complete with value/error exactly once on every path. Packaged tasks and callbacks need ownership after enqueue rejection/shutdown.

Cancellation is cooperative: define interruption points and cleanup/rollback. A stop request does not prove work stopped.

### Parallel Algorithms

Operations under `par`/`par_unseq` must obey required purity/thread/vector safety. Do not mutate shared state, call unsafe APIs, throw where policy terminates, or depend on deterministic order without proof.

Reduction operations must be associative enough for reordered grouping and preserve numeric/error semantics.

### Coroutines

Use an established coroutine runtime abstraction. Define frame/handle ownership, executor affinity, cancellation, exception propagation, continuation scheduling, and destruction of abandoned operations.

Never resume a handle concurrently or after destruction; do not retain references across suspension without owner lifetime guarantees.

### Shared Data And Reclamation

Prefer immutable messages/ownership transfer to shared mutation. For lock-free nodes, choose a proven reclamation scheme (hazard pointers, epochs, reference counting) rather than deleting a node another thread may read.

Avoid false sharing in measured hot counters/queues while preserving portable layout and memory use.

## Verification Focus

- Test start, success, failure, cancellation, queue full/empty, shutdown with pending work, repeated stop, and no enqueue after close.
- Run TSan or platform race tooling on representative shared-state paths when supported.
- Use barriers/latches/hooks to make race schedules deterministic; avoid sleep-only tests.
- Stress repeated execution with deadlines and preserve diagnostics for deadlock/livelock.
- Verify exception/result propagation and resource release after cancellation/shutdown.

## Evidence Focus

Name the owner, invariant/synchronization or memory-order proof, backpressure/shutdown policy, and deterministic/TSan evidence. “Thread-safe” or one passing stress run is not enough.

## Unsafe Defaults

- Detached thread or abandoned future with unclear lifetime/failure.
- Atomic chosen because mutexes seem slow without profiling/proof.
- Relaxed ordering without a happens-before argument.
- Condition-variable wait without predicate/shutdown wakeup.
- Unbounded queue/pool and undefined overload behavior.
- Sleep-based tests as the only concurrency evidence.
