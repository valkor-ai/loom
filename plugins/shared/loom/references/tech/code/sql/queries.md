# SQL Query Quality

This file applies to portable hand-written SQL and query-builder logic. Load the selected provider query overlay when syntax, plan behavior, JSON operators, or index semantics are provider-specific.

## When To Use

- The task changes SELECT/INSERT/UPDATE/DELETE statements, joins, CTEs, recursive queries, aggregations, subqueries, set operations, pagination, reporting queries, or ORM query-builder code.
- Use this when query semantics, result shape, row count, ordering, or database-side filtering affects business behavior.
- If the task only changes schema without changing query behavior, use `sql.schema` instead when selected.
- Do not load this file for an entity-only or migration-only task that does not change query behavior.

## Implementation Focus

- State the intended result shape before writing the query: one row, many rows, grouped rows, paginated rows, existence check, aggregate, or mutation.
- Use explicit column lists for production paths. Avoid `SELECT *` because schema changes can silently change payloads and plan cost.
- Keep joins intentional: join type, cardinality, tenant/security filters, and duplicate behavior must be obvious from the query.
- Use CTEs to clarify multi-step logic or reuse intermediate results. Do not turn every simple query into nested CTEs if a direct query is clearer.
- Use recursive CTEs only for real hierarchy/graph traversal and include a cycle/depth guard where bad data could loop.
- Prefer `EXISTS`/`NOT EXISTS` for presence/absence checks. Use `IN` for small static sets or when the optimizer and dialect make it appropriate.
- Handle NULL explicitly in predicates, sorting, aggregates, and equality checks. Avoid assuming `NULL = NULL` or that aggregate results are never null.
- Pagination must be deterministic. Include stable ordering and avoid unbounded result sets for user-facing list APIs.
- Keep mutation queries idempotent where retries are possible and return/read back the state downstream code needs.
- Parameterize user input through the framework/driver. Do not concatenate values into SQL strings.
- Keep provider-specific operators, casts, conflict syntax, and index hints in the selected dialect overlay. Do not hide a provider dependency inside a supposedly portable query rule.

### Subqueries And Set Operations

- A scalar or correlated subquery in a row-producing path can repeat work for every outer row. Compare it with a grouped join or a window calculation when the result semantics allow that rewrite, and verify duplicate behavior before changing it.
- Use `UNION` only when duplicate elimination is part of the result contract. Use `UNION ALL` when duplicates are valid and the extra sort/distinct work is unnecessary. Align column count, compatible types, nullability, and ordering at the set boundary.
- Portable pivot behavior should use explicit conditional aggregation when the output columns are known. Provider pivot operators, extensions, and dynamic-column generation belong in the selected dialect overlay.

### Mutation Result Boundary

For `INSERT`, `UPDATE`, and `DELETE`, define affected-row semantics, no-op behavior, generated values, and the state that downstream code must read back. Do not infer success from the absence of a driver exception when a zero-row update can mean a missing, stale, unauthorized, or already-completed record.

## Verification Focus

- Test the query against representative fixtures that include empty results, duplicate-prone joins, null values, boundary dates/numbers, and authorization/tenant filters when relevant.
- For mutations, prove affected row count or write/read state, including no-op and invalid-input cases when the task owns them.
- For pagination and sorting, test stable order across multiple rows with same primary sort value.
- For recursive, aggregate, or reporting queries, include fixtures that prove the edge case the query was introduced to handle.
- For query-plan changes, record the provider, query shape, relevant indexes, and plan evidence. Do not claim performance improvement from a query that was never executed against representative data.
- For subquery rewrites and set operations, compare row counts, duplicates, null behavior, and ordering against the prior query on representative fixtures.

## Evidence Focus

- In the evidence summary, name the query decision: result shape, join cardinality, CTE, recursive guard, aggregation, EXISTS, NULL handling, pagination, parameterization, or mutation readback.

## Risks To Avoid

- Selecting SQL references for every backend or API task.
- Replacing a provider-specific query with a different dialect and calling it compatible.
- Using `EXPLAIN ANALYZE` on a mutating statement without a controlled verification boundary.
- Treating a passing mock repository test as proof of provider-specific query behavior.
- Replacing a correlated subquery with a join without checking one-to-many multiplication.
- Using a provider pivot feature when a portable result shape is sufficient and the provider capability is not accepted.
