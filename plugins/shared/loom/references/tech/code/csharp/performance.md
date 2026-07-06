# C# Performance Quality

Use this topic reference when `tech/code/csharp/performance.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes performance-sensitive C# paths, allocation-heavy processing, streams, serialization, LINQ-heavy code, EF queries, caching, AOT/source generation, or benchmarked hotspots.
- Use this when performance is a stated requirement, a measured bottleneck, or a risk introduced by the task.
- Do not introduce low-level optimizations into ordinary business code without evidence or a clear bounded hotspot.

## Implementation Focus

- Start with the simplest correct code, then optimize measured hotspots. Do not use `Span<T>`, `Memory<T>`, pooling, unsafe code, or source generators only to appear "high performance".
- Avoid multiple enumeration of `IEnumerable<T>` when the source can be expensive, streaming, or side-effectful. Materialize once only when the data size is bounded and reuse is needed.
- Use appropriate collections: dictionary/set for lookup/uniqueness, list with known capacity for accumulation, frozen collections for static read-heavy maps in supported target frameworks.
- Keep async allocation choices honest. Use `ValueTask<T>` only for frequently synchronous paths and only when consumers understand single-await semantics.
- Return rented buffers to `ArrayPool<T>` in `finally`, and do not expose pooled arrays beyond their ownership scope.
- For streams and large payloads, avoid buffering whole content into memory unless the maximum size is bounded by contract.
- Optimize EF queries through projections, `AsNoTracking`, pagination, split queries, compiled queries, or indexes before adding application-level caching.
- Apply response caching/compression only when semantics and headers make it safe for the endpoint and users.
- Use JSON source generation or AOT-oriented changes only when the project targets Native AOT, cold-start constraints, or reflection trimming risks.
- Keep performance claims tied to tests, benchmarks, profiling, or query evidence rather than intuition.

## Verification Focus

- Run normal `dotnet build`/`dotnet test` first; optimized code still needs correctness coverage.
- For a performance task, run the repository's BenchmarkDotNet benchmark, profiler, query plan, or targeted measurement and record the comparison.
- For allocation-sensitive changes, verify buffer ownership, stream disposal, and no use-after-return of pooled arrays.
- For EF performance changes, test query output and include projection/query-shape evidence when feasible.

## Evidence Notes

- Record `csharp.performance` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/csharp/performance.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the performance decision: measured hotspot, collection choice, LINQ enumeration, pooled buffer, stream handling, EF query optimization, caching, source generation, or benchmark result.
