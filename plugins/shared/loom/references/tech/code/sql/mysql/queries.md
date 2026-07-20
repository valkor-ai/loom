# MySQL Query Semantics

Use this file with `tech/code/sql/queries.md` when a MySQL task owns query behavior, repository queries, CRUD reads/writes, pagination, JSON access, or query-plan changes.

## When To Use

- Confirm the MySQL version, driver/ORM query mode, collation, and existing index definitions before using provider-specific syntax.
- Keep the query result shape, authorization filters, and deterministic ordering from the common SQL contract.
- Do not add a provider-only query feature when the accepted contract can be implemented portably without a measured need.

## Implementation Focus

- Keep comparisons type-correct. Avoid implicit conversion between numeric, text, temporal, and JSON values because it can change index use and results.
- Keep date/time predicates sargable. Use range boundaries rather than applying a function to an indexed column when the business interval is known.
- Treat collation and case sensitivity as part of search behavior. A text query that changes collation can change both results and index use.
- Use JSON operators and generated-column access only when the repository's MySQL version supports them and the field is part of the accepted query contract.
- Use explicit conflict targets and update columns for MySQL upsert behavior. Preserve idempotency and affected-row semantics for retries.

## Index And Pagination Alignment

- Design composite indexes from the actual equality, range, join, and ordering predicates. Do not add every form field to an index.
- Use deterministic ordering with a unique tie-breaker for offset or keyset pagination.
- Use full-text or spatial indexes only when the task owns that search/geospatial behavior and includes representative verification.
- Inspect the MySQL execution plan for a performance task. A new index is not evidence of improvement by itself.

## Plan Review

- Check access type, examined rows, chosen index, residual filtering, sort work, and join cardinality for the changed query.
- Compare the plan with the expected filter and ordering path. A query can return correct rows while scanning an unsafe amount of data.
- Do not force an index hint until the repository has a provider-specific reason and evidence that the optimizer choice is harmful.
- Treat collation, casts, functions on indexed columns, and implicit conversions as possible causes of an index not being used.
- Record representative data assumptions when the plan depends on cardinality or value distribution.

## Read And Write Boundary

- A repository query must return the fields required by the service or API contract without exposing storage-only fields.
- A mutation query must make its affected-row and no-op behavior clear to the caller.
- Keep authorization, tenant, and soft-delete predicates in the same query boundary that owns the read or write.
- For retries, preserve the conflict target and make duplicate execution observable and safe.

## Review Questions

- What exact user or service behavior requires this query?
- Which MySQL feature is being used, and what version evidence supports it?
- Which index or ordering rule does the query depend on?
- Which empty, duplicate, null, and boundary cases prove the result shape?

## Transactions And Mutations

- Keep multi-row mutations inside the application transaction boundary defined by Architecture.
- Return or read back the state required by downstream API/UI code after a mutation.
- If a deadlock or transient lock failure can be retried, make the operation idempotent and keep retry policy in the owning application layer.

## Verification Focus

- Test empty results, duplicate-prone joins, nulls, boundary dates/numbers, stable pagination, and business filters relevant to the query.
- For a plan or index change, record the provider version, query shape, relevant index, and plan observation.
- For writes, prove affected-row behavior and a read-back result against MySQL when the provider behavior is part of the change.

## Evidence Focus

- In the evidence summary, name the query decision made: result shape, predicate/index alignment, pagination, provider operator, affected-row behavior, or read-back proof.

## Risks To Avoid

- Using a generic JOIN-over-subquery rule without checking result cardinality and the MySQL plan.
- Calling a query successful because a mocked repository returned the expected object.
- Using a leading-wildcard search without an accepted full-text or alternate search design.
- Claiming MySQL compatibility from a different provider's query test.
