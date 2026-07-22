# Redis Atomicity And Coordination

## When To Use

Use this reference when the task owns Redis locks, rate limits, compare-and-set behavior, atomic counters, deduplication, or coordination between workers.

## Implementation Focus

- State the key scope, owner identity, lease duration, and release rule before adding a lock.
- Use `SET key value NX EX` or an equivalent provider-safe primitive for a simple lease.
- Release a lock only when the stored owner token matches; a blind `DEL` can release another worker's lock.
- Use MULTI/EXEC, WATCH, or a Lua script only when the operation needs an atomic boundary across multiple Redis commands.
- Keep Lua scripts short, deterministic, bounded, and versioned with the application code.
- Add rate-limit keys with an explicit identity and time window; do not use a global key for user-scoped limits.
- Set expiration on lock, deduplication, and rate-limit keys so abandoned state can recover.
- Define the behavior when Redis is unavailable, including whether the operation fails closed or uses a bounded fallback.

## Idempotency Boundary

An idempotency key must identify the operation and caller scope. Store a short-lived claim or completed result only when the operation's retry semantics require it. Claiming a key does not make an external side effect atomic; record the side-effect boundary and reconciliation behavior separately.

## Verification Focus

- Competing workers cannot both acquire the same lease under the declared race.
- A stale owner cannot release a newer owner's lock.
- Expired claims, locks, and rate limits recover without manual cleanup.
- Repeated messages or requests produce one durable business effect where required.
- Script, transaction, timeout, and unavailable-provider behavior are covered.

## Evidence Focus

Record the key scope, owner token, expiration, atomic command boundary, failure mode, and focused concurrency or retry evidence.

Show the command or script boundary in evidence. Do not claim a distributed guarantee from a unit test that never runs two competing workers or exercises expiration.

## Unsafe Defaults

- Treating a Redis lock as a replacement for a database constraint.
- Using `SETNX` without expiration.
- Blind `DEL` for lock release.
- Retrying a non-idempotent external effect because Redis accepted a claim.
- Implementing a distributed guarantee with only a process-local mutex.
- Using a long lease without renewal, cancellation, or an explicit maximum work duration.
- Treating an expired lock as proof that the previous worker stopped executing.
- Hiding lock contention and rate-limit rejection as generic server errors.
