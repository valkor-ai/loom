# SQL Server Transaction Behavior

Use this file with `tech/code/sql/schema.md` or `tech/code/sql/queries.md` when a task owns SQL Server transaction boundaries, locking, retry behavior, or multi-row persistence changes.

## When To Use

- Apply these rules to transactional application code and persistence tests.
- Confirm the actual SQL Server transaction boundary, isolation configuration, driver behavior, and connection scope used by the application.

## Implementation Focus

- Keep each transaction limited to state changes that must commit or roll back together.
- Select the isolation level or row-versioning behavior only when the business invariant requires it. Record the reason and verify it against SQL Server.
- Keep lock acquisition order stable. Use lock hints only for a named provider-specific invariant and document their interaction with isolation and timeout behavior.
- Handle deadlock error 1205 and transient lock failures at the application boundary with bounded retry and idempotency. Do not retry every database exception.
- Preserve unique, foreign-key, and state-transition invariants in the database and domain service. UI checks are not transaction protection.

## Verification Focus

- Test commit, rollback, duplicate submission, invalid transition, deadlock/lock timeout, and retry branches owned by the task.
- Run transaction-sensitive tests against SQL Server or the repository's provider-compatible path.
- Record transaction boundary, isolation/locking decision, retry behavior, and provider evidence.

## Evidence Focus

- Name the transaction boundary, invariant, retry classification, rollback behavior, or SQL Server lock result verified.

## Failure Matrix

- Constraint violation: return the repository's validation or conflict error and do not retry blindly.
- Deadlock or transient lock failure: retry only when the operation is idempotent and the owning application layer has a bounded policy.
- Duplicate request: preserve declared uniqueness or idempotency and avoid a second durable effect.
- Partial downstream failure: keep the transaction limited to database state and record compensation outside it when required.
- Request cancellation: release the transaction and connection through the existing framework boundary.

## ORM And Driver Boundary

- Confirm that the transaction annotation, unit-of-work, or connection scope includes every write that must be atomic.
- Do not open a second unmanaged connection inside a transaction-owned service method.
- Verify rollback and affected-row behavior through the actual data-access path, not only a mocked service.

## Review Questions

- Which writes must commit together, and which are intentionally outside the boundary?
- What errors are permanent, transient, or retryable?
- What makes retry or lock acquisition safe for this mutation?
- Which SQL Server behavior and version were verified rather than assumed?

## Boundary Checklist

- Identify the application operation that owns the transaction.
- Identify durable constraints protecting the same invariant during retries.
- Keep external calls and user interaction outside the database transaction.
- State expected behavior after rollback and retry.

## Risks To Avoid

- Using a mock transaction as the only proof of SQL Server locking or isolation behavior.
- Retrying every database exception without classifying permanent and transient failures.
- Holding transactions open across HTTP calls, browser actions, or unbounded loops.
