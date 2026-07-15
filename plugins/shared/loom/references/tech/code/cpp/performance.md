# C++ Runtime Performance And Memory Layout

## When To Use

Use this reference only for an explicit measured CPU, allocation, memory, throughput, latency, binary-size, cache, SIMD, or runtime-resource bottleneck owned by the task.

## Implementation Focus

### Establish The Measurement

Name workload, input distribution/size, concurrency, build type, compiler/flags, hardware/platform, warmup, repetitions, statistic, and correctness output before changing code.

Profile first with the repository's tools. Avoid optimizing debug builds, synthetic tiny data, or a microbenchmark whose work is removed/constant-folded.

Set a target or regression bound. Keep benchmark setup/I/O out of the measured section and use anti-optimization facilities.

### Algorithm And Data Movement

Improve complexity, reduce redundant work/I/O/serialization, and localize state before micro-optimization.

Use values/moves/copy elision naturally. Measure copies and allocations before replacing clear ownership with views/references. Mark safe moves `noexcept` when containers otherwise copy.

Reserve/pre-size only when cardinality is known and capacity/memory tradeoff is acceptable. Avoid repeated shrink/growth churn.

### Layout And Locality

Choose contiguous/flat/node/SoA/AoS layout from access pattern, mutation, iteration, stable-address, and memory requirements. Account for padding, alignment, false sharing, NUMA, and object lifetime.

Do not use `#pragma pack` or over-alignment casually; misalignment can be undefined/slow and layout changes can break ABI/file/wire contracts.

Bound caches/pools/arenas and define reset/destruction/thread safety. Custom allocators must satisfy allocator semantics and object alignment/construction/destruction.

### SIMD And Hardware Features

Prefer compiler auto-vectorization and portable algorithms first. Intrinsics require feature detection/dispatch, scalar or lower-ISA fallback, correct alignment/unaligned loads, edge/tail handling, NaN/overflow/rounding semantics, and supported architectures.

Do not compile an AVX/NEON path as the only binary path unless the deployment CPU baseline guarantees it. Keep dispatch initialization thread-safe.

Inspect generated code only to answer a measured question, not as a substitute for end-to-end benchmarks.

### Branching, Virtual Dispatch, And Indirection

Change branch layout, virtual dispatch, function wrappers, indirection, or polymorphism only when profiles attribute meaningful cost. Type erasure/devirtualization/template expansion can trade runtime for code size/instruction cache/build time.

Preserve domain/error clarity; fast paths require equivalent slow/error behavior and exception/resource safety.

### Parallelism And Offload

Parallelism must exceed scheduling/synchronization/data-transfer overhead and define deterministic/reduction semantics. More threads do not guarantee lower latency.

GPU/accelerator/SIMD offload requires transfer/setup/fallback/error/device-lifetime behavior and representative end-to-end measurement.

### Memory And Resource Pressure

Measure peak/resident/allocated bytes, fragmentation, retained capacity, and leaks according to the problem. A faster implementation that grows unbounded memory or leaks is not acceptable.

Avoid owning large buffers in long-lived closures/caches; release resources at task/request lifecycle boundaries.

## Verification Focus

- Run correctness tests before and after optimization under normal and optimized builds.
- Record repeatable baseline/after benchmark/profile with environment and variability.
- Run ASan/UBSan and relevant platform tools for custom allocation, alignment, pointer arithmetic, SIMD, or lifetime changes.
- Test empty/small/large/unaligned/tail/fallback/feature-dispatch/error cases.
- Check memory, code size, compile time, and concurrency regressions affected by the optimization.

## Evidence Focus

Report bottleneck, workload/environment, profile, intervention, measured result, variability, and correctness/fallback proof. Source-level “fewer copies” or intrinsic presence does not establish user-relevant improvement.

## Unsafe Defaults

- Optimization selected from prose without measured ownership.
- Views/references/custom pools replacing clear ownership prematurely.
- ISA-specific code without runtime/deploy support and fallback.
- Microbenchmark optimized away or unrepresentative of production.
- Faster median hiding tail latency, memory, correctness, or code-size regression.
- Cache/layout changes made without invalidation/lifetime/ABI analysis.
