# Redis Queues And Streams

## When To Use

Use this reference when the task owns an accepted Redis `queue` or `stream` capability, including producers, workers, acknowledgments, retries, or dead-letter handling.

## Implementation Focus

- Choose List for a simple bounded work queue and Stream for consumer groups, acknowledgments, replay, or pending-entry inspection.
- Give every message a stable idempotency identity and a versioned payload envelope.
- Define when a message is considered claimed, processed, acknowledged, retried, or permanently failed.
- Bound payload size, queue depth, consumer concurrency, retry count, and retention.
- For Streams, use consumer groups and acknowledge only after the business effect is committed.
- For Lists, define the visibility and recovery behavior around worker crashes; a blocking pop alone is not durable acknowledgment.
- Use a Sorted Set or an explicit scheduled store for delayed retries, with a bounded backoff and dead-letter destination.
- Keep retries separate from ordinary redelivery and classify permanent validation failures.
- Use an idempotency String or Hash when message redelivery could repeat an external or durable effect.

## Failure Boundary

Redis message delivery does not make a database write and an external side effect atomic. Define the outbox, transaction, acknowledgment, or reconciliation boundary that the application actually owns.

If the queue is optional, the user-facing operation must have an accepted fallback. If the queue is required, readiness and failure must be visible before the application reports success.

## Verification Focus

- A worker acknowledges only after the intended effect succeeds.
- A crash before acknowledgment produces a bounded retry or pending state.
- Duplicate delivery does not repeat an idempotent business effect.
- Retry backoff, maximum attempts, dead-letter handling, and permanent failures are explicit.
- Payload validation, version handling, cancellation, and shutdown drain behavior are covered.

## Evidence Focus

Name the selected List or Stream boundary, message identity, acknowledgment point, retry policy, idempotency store, and evidence for crash or duplicate delivery.

Include the worker shutdown and redelivery evidence when those lifecycle rules are owned by the task. A producer-only test does not prove delivery or retry safety.

## Unsafe Defaults

- Treating `LPUSH`/`RPOP` as a reliable acknowledged queue without recovery design.
- Acknowledging before the database or external effect commits.
- Infinite retries for malformed or permanently forbidden messages.
- Unbounded Streams, Lists, payloads, or worker concurrency.
- Assuming Pub/Sub provides durable delivery or replay.
