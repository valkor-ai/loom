# PostgreSQL Transaction Behavior

Use this file with `tech/code/sql/schema.md` or `tech/code/sql/queries.md` when a task owns PostgreSQL transaction boundaries, locking, retry behavior, or multi-row persistence changes.

## Applicability Boundary

- Apply these rules to transactional application code and persistence tests. Database server operations and pool tuning are outside this reference.
- Confirm the ORM/driver transaction boundary and the PostgreSQL provider path used by the application.

## Implementation Focus

- Keep each transaction limited to state changes that must commit or roll back together.
- Select an isolation level only when the business invariant requires behavior beyond the repository default. Record the reason and verify it against PostgreSQL.
- Define lock ownership, acquisition order, timeout, release, and failure behavior for row or advisory locks.
- Handle serialization, deadlock, and transient lock errors at the application boundary with bounded retry and idempotency.
- Keep domain invariants in the service/application layer and durable constraints in the database. Do not use a lock to replace missing validation.

## Verification And Evidence

- Test commit, rollback, duplicate submission, invalid transition, and relevant serialization/lock branches.
- Run transaction-sensitive tests against PostgreSQL or the repository's provider-compatible test path.
- Record the transaction boundary, isolation or lock decision, retry behavior, and provider evidence in the result.

## Anti-Patterns

- Using a mock transaction as the only proof of PostgreSQL locking or constraint behavior.
- Retrying every database exception without classifying transient and permanent failures.
- Holding transactions open across HTTP calls, browser actions, or unbounded loops.
