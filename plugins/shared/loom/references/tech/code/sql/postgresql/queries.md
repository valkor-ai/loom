# PostgreSQL Query Semantics

Use this file with `tech/code/sql/queries.md` when a PostgreSQL task owns query behavior, repository queries, CRUD reads/writes, pagination, JSONB access, or query-plan changes.

## Applicability Boundary

- Confirm the PostgreSQL version, driver/ORM query mode, extensions, and existing indexes before using provider-specific syntax.
- Keep result shape, authorization filters, and deterministic ordering from the common SQL contract.
- Do not add a PostgreSQL-only feature when a portable implementation satisfies the accepted behavior without a measured need.

## Query And Type Semantics

- Use explicit casts when JSONB, enum, numeric, timestamp, or network values cross a typed application boundary.
- Use JSONB containment and existence operators only when the indexed access path and field semantics are part of the task.
- Treat timestamp timezone semantics as part of the query contract. Do not compare local display values as if they were stored instants.
- Use CTEs for clarity or reuse, and verify materialization behavior against the target PostgreSQL version when plan cost matters.
- Preserve upsert conflict targets, update columns, no-op behavior, and idempotency explicitly.

## Index And Pagination Alignment

- Choose B-tree, GIN, GiST, BRIN, partial, or covering indexes from actual predicates, ordering, data distribution, and provider support.
- A partial index must match the query predicate and active-record semantics; otherwise it is not a usable proof of performance.
- Use deterministic ordering with a unique tie-breaker for offset or keyset pagination.
- Use `EXPLAIN` or controlled `EXPLAIN ANALYZE` for performance work. Do not execute mutating statements merely to produce a plan.

## Transactions And Mutations

- Keep multi-row mutations inside the application transaction boundary defined by Architecture.
- If advisory or row locks are required by the current business rule, define ownership, timeout, release, and failure behavior explicitly.
- Handle serialization, deadlock, and transient lock failures with bounded retry and idempotency in the owning application layer.

## Verification And Evidence

- Test empty results, nulls, duplicate-prone joins, timestamp boundaries, JSONB predicates, stable pagination, and business filters relevant to the query.
- For a plan or index change, record PostgreSQL version, extension/index choice, query shape, and plan observation.
- For writes, prove affected-row behavior and read-back against PostgreSQL when provider behavior is part of the change.

## Anti-Patterns

- Treating JSONB, GIN, partial indexes, or CTE materialization as free defaults.
- Using `EXPLAIN ANALYZE` on an uncontrolled mutation.
- Claiming PostgreSQL compatibility from a different provider's query test.
