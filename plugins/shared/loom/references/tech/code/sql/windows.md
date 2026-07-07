# SQL Window Function Quality

This file applies to ranking, analytic, and per-partition SQL calculations.

## When To Use

- The task changes ranking, top-N-per-group, deduplication, running totals, moving averages, lag/lead comparisons, cohort analysis, sessionization, percentiles, or other analytic SQL.
- Use this when a window function can replace self-joins, repeated subqueries, or application-side row-by-row calculations.
- If the query is a simple aggregate grouped query with no row-level analytic result, do not add windows unnecessarily.

## Implementation Focus

- Define `PARTITION BY`, `ORDER BY`, and frame semantics explicitly. The default frame is often wrong for `LAST_VALUE`, running totals, and centered windows.
- Choose ranking functions deliberately: `ROW_NUMBER` for one deterministic row, `RANK` when ties leave gaps, `DENSE_RANK` when ties share rank without gaps, and `NTILE` for buckets.
- For top-N-per-group and deduplication, include a stable tie-breaker in the window `ORDER BY` so results do not change between executions.
- Use `LAG`/`LEAD` for previous/next row comparisons and handle first/last row nulls deliberately.
- Use aggregate windows for running totals, rolling averages, percent-of-total, and cohort metrics when you need both detail rows and aggregate context.
- Filter at the correct stage. Predicates before the window change the partition; predicates after the window select from ranked/calculated results.
- Avoid multiple expensive window passes when the same partition/order can serve several calculations. Consolidate compatible windows where it improves clarity and cost.
- Check dialect support for frame syntax, filtered aggregates, percentile functions, and date-range windows before using them.
- Keep large partitions and sort requirements visible. Add or verify supporting indexes when window calculations are on hot paths.

## Verification Focus

- Test fixtures with ties, empty partitions, first/last row edges, null values, and multiple rows sharing the same order value.
- For running/rolling calculations, verify the exact frame behavior at partition boundaries.
- For deduplication/top-N, prove deterministic selection with tie-breakers.
- For large analytic paths, review query plan or timing evidence when feasible.

## Evidence Focus

- In the evidence summary, name the window decision: partition/order/frame, ranking choice, tie-breaker, lag/lead null handling, running/rolling aggregate, filter stage, dialect support, or plan proof.
