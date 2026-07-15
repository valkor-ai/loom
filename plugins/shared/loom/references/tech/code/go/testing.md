# Go Test And Verification Design

## When To Use

Use this reference only when the task explicitly owns Go tests, fixtures/fakes, integration/tagged tests, race checks, fuzzing, benchmarks, golden output, or test infrastructure.

## Implementation Focus

### Public Behavior And Cases

Test exported/consumer-visible functions, handlers, services, workers, repositories, commands, and adapters through results/effects. Avoid private call-order tests that freeze implementation.

Use table-driven tests for meaningful input partitions and subtests with scenario names. Do not force every test into a table when setup/assertions differ substantially.

Cover success, invalid/empty/boundary, typed/wrapped error, cancellation/deadline, repeated/concurrent operation, and cleanup changed by the task.

Use `errors.Is`/`errors.As`, structured response fields, and public output; compare exact error text only when it is user/public contract.

### Helpers, Cleanup, And Isolation

Mark helpers with `t.Helper`, fail at the caller, and register `t.Cleanup` after successful acquisition. Check cleanup errors when they affect correctness.

Use `t.TempDir`, `t.Setenv`, unique DB/resource names, fake clocks/random/IDs, and dependency injection. Restore global log/output/timezone/current directory/signal/registry state.

Use `t.Parallel` only after proving no shared env, package globals, singleton fakes, temp collisions, ports, DB records, or ordering. Parent/child parallel scheduling must be understood.

### Fakes, HTTP, And External Boundaries

Prefer small stateful fakes for consumer interfaces and `httptest.Server`/`ResponseRecorder` for HTTP behavior where appropriate. Assert method/path/query/header/body/cancellation and close bodies.

Do not mock the function/package under test or require a mocking framework for tiny interfaces. Generated mocks follow pinned repository tooling and generated-file policy.

Integration tests using DB/queues/cloud/filesystems run behind established tags/commands and clean state. Unit tests must not unexpectedly need network, Docker, credentials, or local services.

### Async And Concurrency

Use channels/barriers/hooks/fake clocks to control ordering. Avoid sleep-only synchronization; deadlines protect against hangs but do not prove desired scheduling.

Assert goroutine shutdown, channel close/queue behavior, cancellation/error propagation, and no work after stop. Run race detector on affected packages and keep tests representative enough to execute the shared state.

Avoid `t.Fatal`/`FailNow` from non-test goroutines; send results/errors back to the test owner.

### Fuzz And Property Tests

Use fuzzing for parsers, decoders, protocol handlers, validators, path/URL logic, and state transitions accepting broad untrusted input.

Fuzz targets are deterministic, bounded, reset global state, avoid network/unbounded allocation, and assert invariants/no panic. Promote minimized failures to fixed regression seeds/cases.

Property tests fit round-trip, ordering, idempotency, serialization, and algebraic invariants; preserve domain constraints in generators.

### Golden Files And Snapshots

Use `testdata` golden files for stable generated/rendered/protocol output with explicit update flag/workflow and reviewed diffs. Normalize only nondeterministic fields intentionally.

Avoid huge volatile snapshots or automatic updates in normal test runs. Assert semantics separately where formatting alone cannot prove behavior.

### Benchmarks And Examples

Benchmarks use representative setup, `b.ResetTimer`, `b.ReportAllocs`, sinks/anti-optimization, sub-benchmarks, and correctness checks outside measured loops.

Run enough samples/benchstat/profiling for claims; benchmarks do not replace correctness tests.

Examples compile and document public use; output examples require deterministic output. Avoid examples that depend on network/local config.

### Coverage And Tooling

Coverage guides untested branches but no universal percentage proves quality. Check test discovery/package/tag selection and avoid reporting cached/stale results as current evidence when relevant.

Run vet/staticcheck/golangci-lint according to repository policy; tools supplement behavior tests.

## Verification Focus

- Run the narrow affected package tests, then affected module lanes according to blast radius.
- Run targeted `-race`, tagged integration, fuzz regression, or benchmark commands only for owned risks.
- Verify tests are discovered and no cleanup/goroutine/global state leaks under repetition/parallel execution.
- Exercise release/OS/arch/tag behavior when build constraints change semantics.
- Record unavailable external infrastructure precisely without claiming it passed.

## Evidence Focus

Name behavior/invariant, test layer/tag/runtime, and assertion/tool result. Test count, coverage percentage, benchmark speed, or `go test` without package/tag context is weak evidence.

## Unsafe Defaults

- Go testing loaded for every implementation task.
- Every test forced into a table or marked parallel.
- Sleep-only concurrency synchronization.
- Unit tests unexpectedly requiring external infrastructure.
- Generated mocks replacing small meaningful fakes.
- Golden files auto-updated or used for semantic behavior alone.
- Coverage target treated as completion proof.
