# SQL Server Query Semantics

Use this file with `tech/code/sql/queries.md` when a SQL Server task owns query behavior, repository queries, CRUD reads/writes, pagination, JSON access, or query-plan changes.

## When To Use

- Confirm SQL Server version, compatibility level, driver/ORM query mode, and existing indexes before using provider-specific syntax.
- Preserve the common SQL result shape, authorization filters, and deterministic ordering.
- Do not add a SQL Server-only feature when a portable query satisfies the accepted behavior without a measured need.

## Implementation Focus

- Use `OFFSET ... FETCH` for the accepted paginated contract and include a unique tie-breaker in the ordering. Keep `TOP` semantics explicit when a bounded result is required.
- Use `COALESCE` or `ISNULL` deliberately; their type precedence and nullability inference can differ in expressions and computed columns.
- Use `STRING_AGG`, `JSON_VALUE`, `JSON_QUERY`, or `OPENJSON` only when the provider version, typed result shape, and indexed access path are accepted.
- Preserve affected-row and no-op behavior for mutations. If an upsert is required, define the conflict key, update columns, concurrency behavior, and retry boundary explicitly; do not assume `MERGE` is safe for every workload.
- Keep casts, collation, implicit conversions, and functions on indexed columns visible because they can change both result semantics and access plans.

## Index And Pagination Alignment

- Design indexes from equality, range, join, and ordering predicates. Filtered indexes and included columns must match the query predicate and result shape.
- Inspect the SQL Server execution plan for performance work. A correct result or a newly created index is not proof of a lower scan or sort cost.
- Record representative data assumptions when cardinality, parameter sensitivity, or plan choice depends on distribution.

## Plan Review

- Check access method, estimated versus actual rows, join cardinality, residual predicates, sort/spill work, and implicit conversions.
- Compare the plan with the expected filter and ordering path. Do not force an index or hint until provider evidence shows the optimizer choice is harmful for the owned workload.

## Read And Write Boundary

- Return only fields required by the service or API contract; do not expose storage-only columns.
- Keep authorization, tenant, and soft-delete predicates in the query boundary that owns the read or write.
- For retries, preserve the uniqueness/conflict rule and make duplicate execution observable and safe.

## Review Questions

- What exact behavior requires this query?
- Which SQL Server feature and compatibility level support it?
- Which index, ordering, cast, JSON path, or affected-row rule does the query depend on?
- Which empty, duplicate, null, timestamp, and boundary cases prove the result shape?

## Transactions And Mutations

- Keep multi-row mutations inside the application transaction boundary defined by Architecture.
- Define locking and isolation only when the invariant requires it. Keep lock hints out of portable query code.
- Handle deadlock and transient failures with bounded retry and idempotency in the owning application layer.

## Verification Focus

- Test empty results, duplicate-prone joins, nulls, boundary dates/numbers, stable pagination, JSON extraction, and business filters relevant to the query.
- For a plan or index change, record SQL Server version, compatibility level, query shape, relevant index, and plan observation.
- For writes, prove affected-row behavior and read-back against SQL Server when provider behavior is part of the change.

## Evidence Focus

- Name the query decision proved: result shape, predicate/index alignment, pagination, provider function, affected-row behavior, or read-back proof.

## Risks To Avoid

- Relying on implicit conversion or parameter typing for a hot predicate.
- Using `MERGE`, hints, or JSON functions without a provider-specific contract and evidence.
- Claiming SQL Server compatibility from a different provider's query test.
