# MySQL Transaction Behavior

Use this file with `tech/code/sql/schema.md` or `tech/code/sql/queries.md` when a task owns MySQL transaction boundaries, locking, retry behavior, or multi-row persistence changes.

## Applicability Boundary

- Apply these rules to transactional application code and persistence tests.
- Confirm that the affected tables use a transactional engine and that the ORM/driver transaction boundary is the one used by the application.

## Implementation Focus

- Keep each transaction limited to the state changes that must commit or roll back together.
- Define the isolation level only when the business invariant requires behavior beyond the repository default. Record the reason and verify the provider behavior.
- Make lock order stable across competing workflows. Avoid holding a transaction open while waiting on unrelated network or user interaction.
- Handle deadlock and transient lock errors at the application boundary with bounded retry and idempotency. Do not blindly retry non-idempotent mutations.
- Preserve unique, foreign-key, and state-transition invariants in the database and domain service. UI checks are not transaction protection.

## Verification And Evidence

- Test commit, rollback, duplicate submission, invalid transition, and relevant lock/deadlock branches.
- Run transaction-sensitive tests against MySQL or the repository's provider-compatible test path.
- Record the transaction boundary, isolation decision, retry behavior, and provider evidence in the result.

## Anti-Patterns

- Using a mock transaction as the only proof of MySQL locking or constraint behavior.
- Retrying every database exception without classifying transient and permanent failures.
- Leaving transactions open across HTTP calls, browser actions, or unbounded loops.
