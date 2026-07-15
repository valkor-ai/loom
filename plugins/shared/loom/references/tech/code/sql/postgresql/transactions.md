# PostgreSQL Transaction Behavior

Use this file with `tech/code/sql/schema.md` or `tech/code/sql/queries.md` when a task owns PostgreSQL transaction boundaries, locking, retry behavior, or multi-row persistence changes.

## When To Use

- Apply these rules to transactional application code and persistence tests. Database server operations and pool tuning are outside this reference.
- Confirm the ORM/driver transaction boundary and the PostgreSQL provider path used by the application.

## Implementation Focus

- Keep each transaction limited to state changes that must commit or roll back together.
- Select an isolation level only when the business invariant requires behavior beyond the repository default. Record the reason and verify it against PostgreSQL.
- Define lock ownership, acquisition order, timeout, release, and failure behavior for row or advisory locks.
- Handle serialization, deadlock, and transient lock errors at the application boundary with bounded retry and idempotency.
- Keep domain invariants in the service/application layer and durable constraints in the database. Do not use a lock to replace missing validation.

## Verification Focus

- Test commit, rollback, duplicate submission, invalid transition, and relevant serialization/lock branches.
- Run transaction-sensitive tests against PostgreSQL or the repository's provider-compatible test path.
- Record the transaction boundary, isolation or lock decision, retry behavior, and provider evidence in the result.

## Evidence Focus

- In the evidence summary, name the transaction boundary, invariant, retry classification, rollback behavior, or provider lock result that was verified.

## Failure Matrix

- Constraint violation: return the repository's validation or conflict error and do not retry blindly.
- Serialization, deadlock, or transient lock failure: retry only when the operation is idempotent and the owning application layer has a bounded policy.
- Duplicate request: preserve the declared uniqueness or idempotency result and avoid a second durable effect.
- Partial downstream failure: keep the transaction boundary limited to database state and record compensation outside it when required.
- Request cancellation: release the transaction and database resources through the existing framework boundary.

## ORM And Driver Boundary

- Confirm that the transaction annotation, session, unit-of-work, or connection scope includes every write that must be atomic.
- Do not open a second unmanaged connection inside a transaction-owned service method.
- Define advisory or row lock ownership and release behavior in the application boundary that requested the lock.
- Verify rollback behavior through the repository's actual data-access path, not only a mocked service.

## Review Questions

- Which writes must commit together, and which are intentionally outside the boundary?
- What error classes are permanent, transient, or retryable?
- What makes a retry or lock acquisition safe for this mutation?
- Which PostgreSQL behavior, version, or extension was verified rather than assumed?

## Boundary Checklist

- Identify the service method or repository operation that owns the transaction.
- Identify the durable constraints that protect the same invariant if the application retries.
- Keep external calls and user interaction outside the database transaction.
- State the expected behavior after rollback and after a retry.

## Risks To Avoid

- Using a mock transaction as the only proof of PostgreSQL locking or constraint behavior.
- Retrying every database exception without classifying transient and permanent failures.
- Holding transactions open across HTTP calls, browser actions, or unbounded loops.
