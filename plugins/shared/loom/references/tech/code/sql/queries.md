# SQL Query Quality

This file applies to hand-written SQL and query-builder logic.

## When To Use

- The task changes SELECT/INSERT/UPDATE/DELETE statements, joins, CTEs, recursive queries, aggregations, subqueries, set operations, pagination, reporting queries, or ORM query-builder code.
- Use this when query semantics, result shape, row count, ordering, or database-side filtering affects business behavior.
- If the task only changes schema without changing query behavior, use `sql.schema` instead when selected.

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

## Verification Focus

- Test the query against representative fixtures that include empty results, duplicate-prone joins, null values, boundary dates/numbers, and authorization/tenant filters when relevant.
- For mutations, prove affected row count or write/read state, including no-op and invalid-input cases when the task owns them.
- For pagination and sorting, test stable order across multiple rows with same primary sort value.
- For recursive, aggregate, or reporting queries, include fixtures that prove the edge case the query was introduced to handle.

## Evidence Focus

- In the evidence summary, name the query decision: result shape, join cardinality, CTE, recursive guard, aggregation, EXISTS, NULL handling, pagination, parameterization, or mutation readback.
