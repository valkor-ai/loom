# Oracle Transaction Behavior

Use this file with `tech/code/sql/schema.md` or `tech/code/sql/queries.md` when a task owns Oracle transaction boundaries, locking, retry behavior, or multi-row persistence changes.

## When To Use

- Apply these rules to transactional application code and persistence tests.
- Confirm the actual Oracle transaction boundary, isolation configuration, driver behavior, and connection scope used by the application.

## Implementation Focus

- Keep each transaction limited to state changes that must commit or roll back together.
- Select isolation beyond Oracle's default only when the business invariant requires it. Record the reason and verify it against Oracle.
- Define row-lock ownership, acquisition order, timeout, and release behavior for `SELECT FOR UPDATE` or equivalent locking.
- Handle serialization error ORA-08177, deadlock ORA-00060, and transient failures at the application boundary with bounded retry and idempotency.
- Do not use autonomous transactions to hide an unclear transaction boundary or to publish business effects outside the owning workflow.

## Verification Focus

- Test commit, rollback, duplicate submission, invalid transition, lock conflict, deadlock/serialization, and retry branches owned by the task.
- Run transaction-sensitive tests against Oracle or the repository's provider-compatible path.
- Record transaction boundary, isolation/locking decision, retry behavior, and provider evidence.

## Evidence Focus

- Name the transaction boundary, invariant, retry classification, rollback behavior, or Oracle lock result verified.

## Failure Matrix

- Constraint violation: return the repository's validation or conflict error and do not retry blindly.
- Serialization, deadlock, or transient lock failure: retry only when the operation is idempotent and the owning application layer has a bounded policy.
- Duplicate request: preserve declared uniqueness or idempotency and avoid a second durable effect.
- Partial downstream failure: keep the transaction limited to database state and record compensation outside it when required.
- Request cancellation: release the transaction and connection through the existing framework boundary.

## ORM And Driver Boundary

- Confirm that the transaction annotation, unit-of-work, or connection scope includes every write that must be atomic.
- Do not open a second unmanaged connection inside a transaction-owned service method.
- Verify rollback, generated-key, and affected-row behavior through the actual data-access path, not only a mocked service.

## Review Questions

- Which writes must commit together, and which are intentionally outside the boundary?
- What errors are permanent, transient, or retryable?
- What makes retry or lock acquisition safe for this mutation?
- Which Oracle behavior, version, or compatibility mode was verified rather than assumed?

## Boundary Checklist

- Identify the application operation that owns the transaction.
- Identify durable constraints protecting the same invariant during retries.
- Keep external calls and user interaction outside the database transaction.
- State expected behavior after rollback and retry.

## Risks To Avoid

- Using a mock transaction as the only proof of Oracle locking or isolation behavior.
- Retrying every database exception without classifying permanent and transient failures.
- Holding transactions open across HTTP calls, browser actions, or unbounded loops.
