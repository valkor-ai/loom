# C++ Performance Quality

## When To Use

- The task changes performance-sensitive C++ paths, allocation behavior, memory layout, hot loops, SIMD, allocators, move semantics, cache-sensitive data, or benchmarks.
- Use this when performance is a stated requirement, a measured bottleneck, or a risk introduced by the task.
- Do not add low-level optimization to ordinary business code without evidence.

## Implementation Focus

- Prefer clear ownership and algorithmic improvement before micro-optimizing. Measure before and after when the task claims performance improvement.
- Use move semantics by designing movable resource owners correctly. Mark move operations `noexcept` when required by containers, and do not move from objects still used later.
- Avoid unnecessary copies by passing views/references/spans where lifetime is clear. Do not return references to temporaries or views into expired buffers.
- Reserve or pre-size containers when final size is known and the allocation cost matters.
- Choose data layout based on access pattern. Structure-of-arrays, alignment, padding, and cache-line separation need a real hot path and tests/benchmarks.
- SIMD and compiler intrinsics require feature detection, fallback paths, alignment handling, and correctness tests across edge sizes.
- Custom allocators, arenas, and memory pools must define object construction/destruction, ownership, alignment, thread safety, and reset lifetime. Do not use them only to avoid ordinary allocation.
- Avoid manual prefetching unless profiling shows memory latency and the target compiler/platform benefits from it.
- Keep exception safety in optimized code. Fast paths must not leak resources or leave partially mutated state on failure.
- Benchmark code should live outside normal unit tests and use the repository's benchmark framework.

## Verification Focus

- Run normal build/tests first; optimized code still needs correctness proof.
- Run sanitizer builds when memory ownership, alignment, placement new, custom allocation, or pointer arithmetic changed.
- Run benchmarks/profiling for performance claims and record the measured comparison.
- Test edge sizes, empty inputs, non-aligned data, fallback paths, and error handling for SIMD/allocator/memory layout changes.

## Evidence Focus

- In the evidence summary, name the performance decision: measured hotspot, move/copy reduction, container sizing, data layout, SIMD fallback, allocator ownership, alignment, sanitizer proof, or benchmark result.
