# C++ Verification And Test Design

## When To Use

Use this reference only when the task explicitly owns C++ test implementation, fixture/test-target wiring, fuzz/property tests, sanitizer/static-analysis checks, or benchmarks.

## Implementation Focus

### Select The Boundary

Test public functions/classes/adapters/parsers/services/executables through observable results. Use compile tests for template/type contracts, integration tests for OS/library/network/database boundaries, and runtime/sanitizer tools for lifetime/concurrency behavior.

Follow the repository's Catch2, GoogleTest/Mock, Boost.Test, doctest, CTest, custom harness, or embedded framework. Do not introduce a second framework for one task.

Keep benchmarks separate: tests prove correctness; benchmarks/profile runs support performance claims.

### Cases And Invariants

Cover success, invalid/boundary/empty/large input, null/optional/error states, ownership transfer, copy/move, exception/status behavior, and cleanup changed by the task.

For parsers/protocols, include malformed/truncated/unknown/duplicate/overflow/encoding cases. For stateful code, assert illegal/repeated transitions and rollback/partial failure.

Use parameterized/table tests for meaningful equivalence classes without hiding which case failed.

### Fixtures And Isolation

Use RAII fixtures for files, directories, environment, handles, servers, threads, sockets, processes, clocks, databases, and global configuration. Cleanup must run after assertions/exceptions.

Use unique temp resources and isolate tests for parallel execution. Restore locale, environment, working directory, signal handlers, static registries, and singleton state.

Avoid order dependence and test-only sleeps. Inject/control clocks, randomness, scheduling, and I/O where those are part of correctness.

### Mocks And Fakes

Mock external boundaries, not the algorithm/object under test. Prefer small stateful fakes for protocols and use expectations only for calls/order that are public behavior.

Avoid over-specifying private call sequences. Verify outputs, durable effects, resource lifecycle, and externally visible interactions.

### Memory, UB, And Race Tools

Run ASan for memory lifetime/bounds, UBSan for undefined operations, TSan for races, MSan where toolchain/dependencies support instrumented builds, and platform tools for leaks/handles.

Sanitizer configurations must instrument relevant code and use compatible dependencies. A clean run does not prove unexecuted paths.

Use deterministic concurrency hooks/barriers and deadlines plus TSan; one repeated stress test is supporting evidence only.

### Property And Fuzz Testing

Use property tests for invariants across broad generated inputs and fuzzers for parsers, decoders, protocol/state inputs, and memory-safe boundaries with clear dictionaries/seeds/corpus.

Fuzz targets must be deterministic, bounded, leak-safe, reset state each iteration, and convert crashes into reproducible regression seeds.

### Floating Point And Platform Behavior

Choose absolute/relative/ULP tolerance from domain magnitude and algorithm, handling NaN/infinity/signed zero explicitly. Do not use arbitrary epsilon everywhere.

Run platform/compiler-specific behavior on the affected target or record the exact unproved branch. Unit tests on one standard library do not establish all ABI/platform behavior.

### Compile And Public API Tests

Compile public headers standalone where practical and add supported/unsupported template cases through repository compile-fail/static assertion infrastructure.

For libraries, build a consumer or package test to prove exported includes, symbols, transitive dependencies, and standard requirements.

## Verification Focus

- Run the narrow changed test target and prove it is registered/discoverable by the build runner.
- Run relevant sanitizer/static-analysis configurations for the changed risk.
- Repeat concurrency/property/fuzz regression seeds deterministically and retain minimal failing cases.
- Verify release/optimized behavior when debug assertions or optimization could change semantics.
- Record exact compiler/platform/sanitizer limitations without claiming broader coverage.

## Evidence Focus

Name the behavior/invariant, test layer, configuration/compiler/platform, and assertion/tool result. Test count, framework presence, or a green build without executed checks is weak evidence.

## Unsafe Defaults

- Load this reference only when the accepted task owns C++ test creation, test modification, or test-specific verification.
- New test framework added despite repository tooling.
- Private call order asserted instead of public behavior.
- Sleep-only concurrency tests or sanitizer-clean claims over unexecuted paths.
- Benchmarks treated as correctness tests.
- Floating-point exact equality or arbitrary epsilon without domain basis.
