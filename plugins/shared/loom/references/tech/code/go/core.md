# Go Core Quality

Use this topic reference when `tech/code/go/core.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes Go application, library, service, CLI, handler, repository, worker, or shared package code.
- Use this for baseline Go correctness: error handling, context propagation, configuration, exported API behavior, and idiomatic runtime safety.
- If the task only changes generated files, documentation, or non-Go assets, do not expand scope because this reference is available.

## Implementation Focus

- Keep changed code `gofmt` and `go vet` clean. Do not use formatting or generated-code churn to hide small behavior changes.
- Return errors explicitly and wrap them with `%w` when callers need to preserve cause. Do not use `panic` for normal business, validation, I/O, network, or persistence failures.
- Do not ignore errors with `_` unless the ignored error is provably impossible or intentionally irrelevant; add a short local reason when that is not obvious.
- Pass `context.Context` into operations that can block, cross process boundaries, use network/database/filesystem, wait on queues, or run under request/job cancellation. Do not store contexts on structs for long-lived reuse.
- Keep configuration at startup or constructor boundaries. Validate required env/options before starting servers, workers, or destructive jobs.
- Prefer useful zero values for small structs and options where Go convention supports it, but do not rely on zero values for required business configuration such as credentials, database URLs, or externally visible ports.
- Document exported packages, types, functions, and methods when the repository enforces docs or the exported API is consumed outside the package. Comments should describe contract and behavior, not restate the name.
- Use standard library interfaces and types when possible: `io.Reader`, `io.Writer`, `http.Handler`, `context.Context`, `time.Duration`, and concrete errors through `errors.Is` or `errors.As`.
- Keep logging structured and at boundaries. Do not log secrets, full environment dumps, tokens, or sensitive request bodies.
- Avoid reflection for normal mapping, validation, or dispatch unless the task has a framework/interoperability reason and tests cover the reflective path.

## Verification Focus

- Run `go test ./...` or the repository's narrower configured Go test command.
- Run `go vet ./...` or the configured lint command when the task changes exported APIs, formatting-sensitive code, unsafe code, or concurrency.
- Add tests for new error branches, config validation, context cancellation where relevant, and public behavior that changed.
- Confirm no changed path ignores errors, loses cancellation, or relies on panic for recoverable failures.

## Evidence Notes

- Record `go.core` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/go/core.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the Go decision made: error wrapping, context propagation, config validation, exported API docs, standard interface use, logging boundary, or reflection avoidance.
