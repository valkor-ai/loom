# Go Application And Package Delivery

## When To Use

Use this reference for task-owned Go application, service, handler, worker, CLI, library, or domain code. Preserve the module's declared Go/toolchain version, package conventions, error/context policy, generated-code boundaries, and exported API compatibility.

Concurrency, dependency interfaces, generics, module structure, and testing are selected separately.

## Implementation Focus

### Errors And Control Flow

Return errors for recoverable business, validation, I/O, network, persistence, and configuration failures. Reserve panic for impossible programmer/runtime initialization invariants where process failure is intentional.

Wrap with `%w` only when callers should inspect the cause. Use `errors.Is`/`errors.As` against sentinel or typed errors; do not parse message strings or lose causes through `%v`/new text.

Add concise operation/resource context without logging/re-wrapping at every layer. Keep user/API error translation at the owning boundary and prevent secrets/provider details from escaping.

When several independent failures must be retained, use the repository's multi-error policy or `errors.Join` with clear caller semantics rather than returning only the last failure.

Check cleanup/write/close errors when they can change durability or correctness. Deferred error handling must not silently overwrite or discard a prior error.

### Context And Cancellation

Pass `context.Context` as the first parameter to blocking/request/job operations and forward it into DB/HTTP/files/queues/waits. Do not store request contexts in structs or use `context.Background` to escape caller cancellation.

Derive timeouts/deadlines at the boundary that owns the budget and always call cancel. Preserve cancellation/deadline classification and stop new side effects after cancellation.

Do not add context to pure CPU/value helpers solely for consistency; use explicit cancellation where long computation genuinely needs it.

### Values, Pointers, And Ownership

Prefer useful zero values when they are safe. Validate required config/dependencies/business fields in constructors/startup rather than making a dangerous zero value appear usable.

Use pointers when mutation, identity, nil, or copy cost/semantics require them. Avoid pointer-to-interface and pointer fields merely to distinguish omitted input when a dedicated DTO/optional representation is clearer.

Copy structs containing mutexes, atomics, no-copy resources, large buffers, or internal pointers only with explicit semantics. Choose receiver type consistently based on mutation/size/identity/interface method sets.

Slices/maps/channels/functions/interfaces are reference-like descriptors; copying them does not deep-copy data. Define ownership when retaining caller buffers/maps/slices or returning internal mutable storage.

### Collections And Iteration

Preallocate slices/maps when a reliable bound is known, without retaining attacker-controlled/huge capacity. Preserve nil versus empty semantics only when API/encoding/storage contracts distinguish them.

Map iteration order is unspecified. Sort keys or use an ordered representation when output, hashing, tests, pagination, or user display requires determinism.

Avoid mutating shared slices/maps concurrently and account for append reallocation/aliasing. Clone at ownership boundaries when independent mutation is required.

### Resource Lifecycle

Close response bodies, rows, files, streams, timers, tickers, subscriptions, and processes at the owner. Check iteration/scan/final errors (`rows.Err`, scanner errors, command wait) after loops.

Defer cleanup after successful acquisition, not before checking errors. For long loops, explicit per-iteration cleanup may be safer than accumulating defers until function return.

Keep HTTP clients/transports, DB pools, and other concurrency-safe pools long-lived; do not recreate them per operation.

### Boundaries And Data

Validate and normalize external JSON/form/query/header/env/file/message input before domain use. Configure decoder unknown-field, number, size, and trailing-data behavior according to the accepted contract.

Use `time.Time`/`time.Duration`, decimal/money/domain types, URLs/paths, and integer widths according to wire/storage/business semantics. Avoid bare integer duration and float money.

Parameterize SQL and allowlist dynamic identifiers/order fields. Use `io.Reader`/`Writer` streaming with explicit size limits for untrusted or large payloads.

### Configuration, Logging, And Exported APIs

Load and validate configuration at startup/composition boundaries. Distinguish missing/empty/invalid values and avoid silent localhost/insecure production fallback.

Use structured logging at ownership boundaries without tokens, credentials, personal/sensitive payloads, or full environment dumps.

Export the smallest stable API. Document contract when repository/public package policy requires it and avoid leaking internal provider/framework types across package boundaries.

Maintain backward source/behavior/serialization compatibility for published modules unless the accepted change owns a breaking version.

## Verification Focus

- Run focused `go test`, `go vet`, configured lint, and build for affected packages/binaries under the declared Go version.
- Test wrapped error classification, invalid/boundary input, context cancellation/deadline, resource cleanup, and deterministic output.
- Exercise nil/empty/aliasing/map-order and serialization behavior where contract-visible.
- Run race/runtime checks when retained mutable data or lifecycle ownership changes.
- Verify config startup failure and exported/public consumer behavior when changed.

## Evidence Focus

Name the error/context/resource/data ownership decision and public assertion/tool result. `gofmt` or a successful compile does not prove cancellation, cleanup, aliasing, deterministic output, or boundary validation.

## Unsafe Defaults

- Panic for runtime/business/validation failure.
- Error cause lost or message parsed for control flow.
- Context stored on structs or replaced with Background mid-flow.
- Caller/internal mutable slices/maps retained without ownership policy.
- Map order relied on for stable behavior.
- Response/rows/process/cleanup errors ignored.
- Required configuration accepted through unsafe zero values.
