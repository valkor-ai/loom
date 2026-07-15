# PostgreSQL Query Semantics

Use this file with `tech/code/sql/queries.md` when a PostgreSQL task owns query behavior, repository queries, CRUD reads/writes, pagination, JSONB access, or query-plan changes.

## When To Use

- Confirm the PostgreSQL version, driver/ORM query mode, extensions, and existing indexes before using provider-specific syntax.
- Keep result shape, authorization filters, and deterministic ordering from the common SQL contract.
- Do not add a PostgreSQL-only feature when a portable implementation satisfies the accepted behavior without a measured need.

## Implementation Focus

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

## Plan Review

- Check scan type, estimated versus actual rows, join cardinality, sort work, buffer behavior, and index usage for the changed query.
- Compare the plan with the expected predicate and ordering path. Correct results do not prove acceptable access cost.
- Do not add a partial or specialized index until the query predicate, data distribution, and provider evidence justify it.
- Treat casts, timezone conversions, JSONB expressions, functions on indexed columns, and collation as possible causes of a poor plan.
- Record representative data assumptions when the plan depends on cardinality or value distribution.

## Read And Write Boundary

- A repository query must return the fields required by the service or API contract without exposing storage-only fields.
- A mutation query must make affected-row and no-op behavior clear to the caller.
- Keep authorization, tenant, and soft-delete predicates in the same query boundary that owns the read or write.
- For retries, preserve the conflict target and make duplicate execution observable and safe.

## Review Questions

- What exact user or service behavior requires this query?
- Which PostgreSQL feature is being used, and what version or extension evidence supports it?
- Which index, JSONB operator, cast, or ordering rule does the query depend on?
- Which empty, duplicate, null, timestamp, and boundary cases prove the result shape?

## Transactions And Mutations

- Keep multi-row mutations inside the application transaction boundary defined by Architecture.
- If advisory or row locks are required by the current business rule, define ownership, timeout, release, and failure behavior explicitly.
- Handle serialization, deadlock, and transient lock failures with bounded retry and idempotency in the owning application layer.

## Verification Focus

- Test empty results, nulls, duplicate-prone joins, timestamp boundaries, JSONB predicates, stable pagination, and business filters relevant to the query.
- For a plan or index change, record PostgreSQL version, extension/index choice, query shape, and plan observation.
- For writes, prove affected-row behavior and read-back against PostgreSQL when provider behavior is part of the change.

## Evidence Focus

- In the evidence summary, name the query decision made: result shape, predicate/index alignment, JSONB operator, pagination, provider cast, affected-row behavior, or read-back proof.

## Risks To Avoid

- Treating JSONB, GIN, partial indexes, or CTE materialization as free defaults.
- Using `EXPLAIN ANALYZE` on an uncontrolled mutation.
- Claiming PostgreSQL compatibility from a different provider's query test.
