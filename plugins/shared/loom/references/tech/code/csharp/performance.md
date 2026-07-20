# .NET Runtime Performance

## When To Use

Use this reference only when the task owns a measured CPU, allocation, GC, memory, throughput, latency, startup, publish size, query, stream, or runtime-resource bottleneck.

## Implementation Focus

### Measure First

Name workload, input/concurrency, TFM/runtime, OS/hardware/container limits, build tier, warmup, repetitions, statistic, baseline, and correctness output.

Use BenchmarkDotNet for microbenchmarks and appropriate tracing/profilers/counters for application behavior. A debug stopwatch or one request is not reliable evidence.

Check algorithm, query/network I/O, serialization, blocking, contention, and redundant work before low-level allocation tuning.

### Enumeration And Collections

Know whether `IEnumerable<T>` is lazy, repeatable, remote, streaming, or side-effectful. Avoid multiple enumeration; materialize once only when bounded ownership/reuse justifies memory.

Choose list/dictionary/hash/frozen/immutable/concurrent structures from access/mutation/concurrency/lifetime. Pre-size with realistic cardinality and avoid retained oversized capacity.

Use LINQ when clear; remove allocations/enumerations only in measured hot paths and preserve provider translation for `IQueryable`.

### Span, Memory, And Pools

Use spans for synchronous contiguous parsing/formatting/processing where backing lifetime is explicit. They cannot cross async/yield/heap capture.

Use `Memory<T>`/`ReadOnlyMemory<T>` across async only with a clear owner. Do not retain memory over a pooled buffer after return.

Rent bounded buffers/objects from established facilities such as `ArrayPool<T>` only for hot repeated allocation. Return in `finally`, clear sensitive data, avoid double return/use-after-return, cap pooled object capacity, and document thread safety.

Keep `stackalloc` size bounded or conditional; user-controlled/large allocation can overflow the stack.

### Async And Concurrency Cost

Use `ValueTask<T>` only for frequently synchronous hot APIs with measured benefit and consumer semantics (normally one await/consumption). Ordinary `Task<T>` is safer for general APIs.

Avoid fake async (`Task.Run` around I/O), unbounded `WhenAll`, thread-pool starvation, blocking locks, and sync-over-async. Bound channels/concurrency and propagate cancellation.

Measure lock/contention/context-switch costs before replacing safe synchronization.

### Streams, Serialization, And Networking

Stream bounded chunks instead of buffering entire large payloads; preserve cancellation, length limits, disposal, and partial failure behavior.

Use source-generated serialization only when startup/AOT/allocation or trimming needs justify it and every runtime type/options path is registered. Avoid reflection fallbacks hidden until production.

Reuse clients/connections through accepted factories/pools and consume/dispose responses correctly. Compression/caching must preserve endpoint/user semantics.

### GC And Object Lifetime

Measure allocation rate, generations, LOH, pause time, roots, and retained memory. Reducing allocation count is insufficient if retained size or latency worsens.

Avoid long-lived event/static/cache closures retaining request/user/large graphs. Bound caches and unsubscribe/dispose lifecycle owners.

Do not force collections/NoGC regions globally without a proven controlled latency scenario.

### AOT, Trimming, And Startup

Native AOT, ReadyToRun, single-file, trimming, invariant globalization, and source generation trade compatibility, size, startup, build time, reflection/dynamic behavior, and diagnostics.

Test the actual published artifact and deployment platform; ordinary build/tests cannot prove trim/AOT compatibility.

### EF And External Systems

Use provider/task-specific data guidance for query shape. Measure projection/tracking/round trips/query plan/index before application caching or compiled queries.

Include network/database/queue limits when optimizing end-to-end throughput; a faster CPU loop may not affect the real bottleneck.

## Verification Focus

- Run correctness tests before/after under release/published configuration.
- Record repeatable benchmark/profile/counter evidence with environment and variability.
- Test span/pool/stream empty/boundary/error/cancellation and ownership after return/disposal.
- Verify memory retention/GC and concurrency/resource limits under representative load.
- Publish/run trimming/AOT/single-file changes on the target runtime and exercise reflection/serialization/plugins.

## Evidence Focus

Report bottleneck, workload/runtime, measurement, intervention, result/variability, and correctness/resource tradeoff. `Span<T>`, pooling, ValueTask, or source generation presence does not prove meaningful improvement.

## Unsafe Defaults

- Performance reference selected from prose without measured ownership.
- Span/pool/unsafe complexity added to ordinary business code.
- Pooled memory retained after return or sensitive data left uncleared.
- ValueTask used broadly without consumer/lifetime constraints.
- Debug/microbenchmark result generalized to production workload.
- AOT/trimming claimed complete from `dotnet build` only.
