# C++ Testing Quality

## When To Use

- The task adds or changes C++ tests, fixtures, test target wiring, sanitizer/static-analysis verification, benchmarks, mocks, integration tests, or behavior implemented in C++.
- Use this when C++ behavior needs proof through Catch2, GoogleTest, CTest, custom runners, sanitizers, or static analysis.
- Follow the repository's existing test framework and target layout unless the task explicitly owns test infrastructure.

## Implementation Focus

- Test public behavior through functions, classes, adapters, parsers, services, or executable boundaries. Avoid locking tests to private helper order.
- Use fixtures for expensive setup and RAII cleanup. Test fixtures must not leak files, handles, threads, sockets, or global state.
- Cover success, invalid input, boundary values, ownership transfer, exception/error-code paths, and resource cleanup touched by the task.
- Use mocks/fakes for external boundaries such as filesystem, network, clocks, hardware, databases, and process execution. Do not mock the algorithm under test.
- Add sanitizer runs for memory, UB, or thread-risk changes when the project supports ASan/UBSan/TSan/MSan.
- Add static-analysis or warning-clean evidence when headers, templates, casts, ownership, or low-level code changed.
- Keep benchmarks separate from unit tests. Unit tests prove correctness; benchmarks/profiling support performance claims.
- For floating point, use tolerance assertions rather than exact equality unless exactness is the contract.
- For platform-specific code, run or record the platform/compiler target that proves the changed path.

## Verification Focus

- Run the configured build and test command, such as `cmake --build`, `ctest`, Catch2, GoogleTest, or repository scripts.
- Run sanitizer/static-analysis targets when relevant to the change and available locally.
- Confirm test targets are discoverable by the build system when new tests are added.
- Record skipped platform, sanitizer, or integration checks only when unavailable infrastructure prevents them.

## Evidence Focus

- In the evidence summary, name the behavior verified and the C++ commands run.
