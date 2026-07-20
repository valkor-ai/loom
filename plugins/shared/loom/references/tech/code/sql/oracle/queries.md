# Oracle Query Semantics

Use this file with `tech/code/sql/queries.md` when an Oracle task owns query behavior, repository queries, CRUD reads/writes, pagination, JSON access, or query-plan changes.

## When To Use

- Confirm Oracle version, compatibility mode, driver/ORM query mode, optimizer assumptions, and existing indexes before using provider-specific syntax.
- Preserve the common SQL result shape, authorization filters, and deterministic ordering.
- Do not add an Oracle-only feature when a portable query satisfies the accepted behavior without a measured need.

## Implementation Focus

- Use `OFFSET ... FETCH` when supported by the accepted Oracle version; keep older `ROWNUM` pagination isolated and prove its ordering semantics.
- Use `COALESCE` or `NVL` deliberately, including datatype conversion and null behavior. Character empty strings are `NULL` and must not be treated as a separate value.
- Use analytic functions for ranking, top-N-per-group, and comparisons when they preserve the result contract. Keep partition, order, tie-breaker, and frame explicit.
- Use JSON functions, `LISTAGG`, or `MERGE` only when Oracle version, overflow/conflict behavior, and result shape are accepted. For upserts, define conflict key, update columns, no-op behavior, and retry safety explicitly.
- Keep casts, NLS-sensitive conversions, functions on indexed columns, and collation visible because they can change results and access plans.

## Index And Pagination Alignment

- Design indexes from equality, range, join, and ordering predicates. Function-based indexes must match the query expression and migration contract.
- Inspect the Oracle execution plan for performance work. A correct result or a new index is not proof of lower scan, join, or sort cost.
- Record representative data assumptions when cardinality, bind variables, or plan selection depends on distribution.

## Plan Review

- Check access path, estimated versus actual rows where available, join cardinality, sort work, implicit conversions, and function-based index use.
- Compare the plan with the expected filter and ordering path. Do not force a plan or hint until provider evidence shows the optimizer choice is harmful for the owned workload.

## Read And Write Boundary

- Return only fields required by the service or API contract; do not expose storage-only columns.
- Keep authorization, tenant, and soft-delete predicates in the query boundary that owns the read or write.
- For retries, preserve the uniqueness/conflict rule and make duplicate execution observable and safe.

## Review Questions

- What exact behavior requires this query?
- Which Oracle feature and version support it?
- Which index, ordering, cast, JSON path, NLS rule, or affected-row behavior does it depend on?
- Which empty-string/null, duplicate, timestamp, and boundary cases prove the result shape?

## Transactions And Mutations

- Keep multi-row mutations inside the application transaction boundary defined by Architecture.
- Define `SELECT FOR UPDATE` or other locking only for a named invariant, with timeout and retry behavior owned by the application layer.
- Handle serialization, deadlock, and transient failures with bounded retry and idempotency.

## Verification Focus

- Test empty results, null and empty-string values, duplicate-prone joins, boundary dates/numbers, stable pagination, JSON extraction, and business filters relevant to the query.
- For a plan or index change, record Oracle version/mode, query shape, relevant index, and plan observation.
- For writes, prove affected-row behavior and read-back against Oracle when provider behavior is part of the change.

## Evidence Focus

- Name the query decision proved: result shape, predicate/index alignment, pagination, provider function, null semantics, affected-row behavior, or read-back proof.

## Risks To Avoid

- Treating empty strings as ordinary non-null values.
- Relying on session NLS settings or implicit conversion in a query contract.
- Using `MERGE`, hints, or provider functions without version-specific evidence.
- Claiming Oracle compatibility from a different provider's query test.
