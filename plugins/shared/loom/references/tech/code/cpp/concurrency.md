# C++ Concurrency Quality

Use this topic reference when `tech/code/cpp/concurrency.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes threads, thread pools, atomics, mutexes, condition variables, futures, async work, parallel algorithms, queues, coroutines, or shared state.
- Use this when correctness depends on lifecycle, synchronization, memory ordering, shutdown, or data race safety.
- If the changed code is single-threaded and no shared state is introduced, do not add concurrency abstractions.

## Implementation Focus

- Every thread, task, coroutine handle, and worker pool needs an owner and shutdown path. Do not detach threads unless process lifetime ownership is explicit and safe.
- Prefer standard locking primitives with RAII guards before considering atomics or lock-free structures. Use `std::lock_guard`, `std::unique_lock`, `std::scoped_lock`, and predicates for condition variables.
- Condition variables must wait on predicates and handle spurious wakeups. Notify after state changes are visible under the mutex.
- Use atomics for small shared state only when the memory-ordering contract is clear. Default to simpler synchronization when acquire/release/relaxed semantics are not obvious.
- Lock-free structures require ownership, reclamation, ABA, ordering, and testing strategy. Do not introduce them for ordinary queues or maps without measured contention and expertise.
- Avoid holding locks while calling user callbacks, doing blocking I/O, waiting on futures, or performing long CPU work.
- Use `std::scoped_lock` or consistent lock ordering for multiple mutexes to avoid deadlock.
- Bound queues and worker pools when input can grow. Define what happens on shutdown, full queue, task failure, and rejected enqueue.
- Parallel STL algorithms require pure or synchronization-safe operations. Do not mutate shared state inside `par`/`par_unseq` lambdas without clear protection.
- C++ coroutines require clear ownership of coroutine handles and exception propagation. Do not hand-roll coroutine types unless a library/runtime is not suitable.

## Verification Focus

- Run unit/integration tests that cover concurrent behavior and shutdown paths.
- Run ThreadSanitizer or equivalent when shared mutable state, atomics, or thread lifecycle changed and the project supports it.
- Add tests for full/empty queue behavior, cancellation/shutdown, error propagation, and no work after stop where relevant.
- Use timeouts carefully in tests to catch deadlocks without creating flaky sleeps-only checks.

## Evidence Notes

- Record `cpp.concurrency` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/cpp/concurrency.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the concurrency decision: thread ownership, mutex/condition variable, atomic ordering, worker pool, queue backpressure, parallel algorithm, coroutine ownership, or TSan proof.
