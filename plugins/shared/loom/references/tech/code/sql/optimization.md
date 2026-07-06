# SQL Optimization Quality

Use this topic reference when `tech/code/sql/optimization.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. This file applies to query and schema changes made for performance.

## When To Use

- The task changes a slow query, index strategy, query plan, aggregation, search/filter path, pagination performance, materialized view, partitioning, statistics, or database performance regression.
- Use this when the task claims or requires improved runtime behavior, reduced scans, lower latency, or safer behavior at expected data volume.
- If the change is purely functional and no performance path is touched, do not add speculative optimization work.

## Implementation Focus

- Start with the query owner, cardinality, filter/sort pattern, and expected data volume. Do not add indexes before identifying the query they serve.
- Prefer set-based rewrites over row-by-row loops, cursors, or repeated scalar subqueries. Replace repeated correlated work with joins, grouped subqueries, CTEs, or window functions when it preserves semantics.
- Use `EXISTS` for existence checks and avoid `COUNT(*)` when only presence matters.
- Design indexes around equality predicates, range predicates, join keys, and sort order. Include covering columns only when they remove meaningful table lookups for a frequent path.
- Use partial/filtered indexes for common active-state predicates such as non-deleted, active, pending, or tenant-scoped records when the dialect supports them.
- Do not add broad indexes for every column in a filter form. Each index has write cost, migration cost, storage cost, and plan side effects.
- Treat materialized views, partitioning, query hints, denormalization, and cache tables as higher-cost choices. Use them only when normal query/index changes are insufficient or the architecture already has the pattern.
- Keep statistics and plan stability in mind. A plan with large estimated-vs-actual row gaps may need statistics refresh or a more selective predicate, not just another index.
- Preserve correctness while optimizing: null semantics, duplicate rows, tie ordering, authorization/tenant filters, and pagination stability cannot change silently.

## Verification Focus

- Capture plan evidence for optimization claims when the database supports it: `EXPLAIN`, `EXPLAIN ANALYZE`, buffers, estimated vs actual rows, or the repository's equivalent.
- Verify result equivalence with representative fixtures, especially after rewriting joins, aggregations, windows, or pagination.
- Record before/after timing or plan shape when feasible. If production-scale data is unavailable, say what was verified and what remains a volume risk.
- Run affected integration/repository/API tests to ensure the optimized path still returns the expected user-facing data.

## Evidence Notes

- Record `sql.optimization` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/sql/optimization.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the optimization decision: query rewrite, index design, plan finding, set-based replacement, partial index, materialized view, partitioning, statistics, or measured proof.
