# Go Testing Quality

Use this topic reference when `tech/code/go/testing.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task adds or changes Go tests, test fixtures, fakes, integration tests, race-sensitive code, benchmarks, fuzz tests, golden files, or behavior implemented in Go.
- Use this when Go behavior needs proof through the standard test toolchain.
- Follow existing repository test style unless the task explicitly owns test infrastructure.

## Implementation Focus

- Use table-driven tests when the same behavior needs multiple inputs, edge cases, or error branches. Give each case a name that explains the scenario.
- Use subtests to isolate cases. Call `t.Parallel` only when the test has no shared mutable state, global env changes, temp path conflicts, or ordering dependency.
- Mark helper functions with `t.Helper()` and register cleanup through `t.Cleanup` when possible. Avoid cleanup that silently ignores errors.
- Test public behavior through exported functions, handlers, services, repositories, or interfaces. Avoid asserting private call order unless the private function is the delivered unit.
- Use fakes over mocks when behavior is small and stateful. Keep mocks focused on external boundaries such as network, database, filesystem, mail, queues, clocks, and random IDs.
- For error handling, assert with `errors.Is` or `errors.As` when callers depend on wrapped causes. Do not compare full formatted error strings unless the string is user-facing contract.
- Use golden files for stable rendered output, generated text, or protocol payloads. Store them under `testdata` and make updates explicit through the repository's existing update mechanism.
- Add integration tests behind build tags or clear commands when they need external services. Unit tests should not unexpectedly require Docker, cloud credentials, or network access.
- Add benchmarks only for performance-sensitive code or known hotspots. Do not use benchmarks as a substitute for correctness tests.
- Add fuzz tests when parsers, decoders, protocol handlers, or input validators accept broad untrusted input.

## Verification Focus

- Run `go test ./...` or the package-specific command that covers changed code.
- Run `go test -race ./...` or targeted `-race` tests when goroutines, shared state, timers, or worker pools changed.
- Run tagged integration tests, fuzz tests, or benchmarks only when the task touches that risk area or the repository already requires them.
- Confirm fixtures, temp files, env variables, and goroutines are cleaned up after tests.

## Evidence Notes

- Record `go.testing` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/go/testing.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the behavior verified and the Go commands run.
