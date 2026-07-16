# Rust Testing Quality

## When To Use

- The task adds or changes Rust tests, doctests, integration tests, async tests, property tests, snapshots, mocks/fakes, benchmarks, fuzz targets, or behavior implemented in Rust.
- Use this when Rust behavior needs proof through Cargo, clippy, doctests, Miri, benchmarks, or fuzzing.
- Follow existing crate/workspace test layout unless the task explicitly owns test infrastructure.

## Implementation Focus

- Put unit tests near private logic when they need module access; use `tests/` integration tests for public API and end-to-end behavior.
- Add doctests for public APIs where examples should stay compiling and useful. Do not add doctests that require hidden setup unless marked appropriately.
- Test `Result` and `Option` branches without relying on panics unless panic is the contract.
- Use async test macros matching the runtime selected by the crate.
- Prefer small fakes or trait-based test doubles for external boundaries. Use mocking crates only when interaction expectations are important and already accepted by the project.
- Use property-based tests for parsers, encoders, algorithms, state machines, or invariants with broad input space.
- Use snapshots only for stable complex output, and keep review/update workflow explicit.
- Keep benchmarks in `benches/` or the repository benchmark setup. Do not use benchmarks as correctness tests.
- Use fuzzing for untrusted parsers/protocol inputs when the task touches security or robustness-sensitive parsing.
- Clean up temp files, spawned tasks, test databases, environment variables, and global state through RAII/test fixtures.

## Decision Rules

- Choose unit tests for private/pure logic, integration tests for public crate contracts, doctests for public examples, and runtime tests for async/resource behavior. Do not use a mock to claim provider or executor behavior was verified.
- Match async test macros and runtime to the crate. Assert cancellation, task completion, channel closure, and cleanup when those are part of the changed contract.
- Use property tests for parsers, encoders, state machines, or invariants with broad input space; use examples/fixtures for a finite business matrix. Keep generated cases diagnosable.
- Use snapshots only for stable complex output and keep update review explicit. Keep benchmarks and fuzz targets separate from correctness tests.
- Report changed branches and risk evidence rather than imposing a universal coverage percentage. Miri, sanitizer, benchmark, or fuzz evidence should be added when the changed risk actually requires it.

## Verification Focus

- Run `cargo test` for the changed crate/workspace or a narrower configured command.
- Run `cargo fmt --check` or `cargo fmt`, and `cargo clippy --all-targets --all-features` when configured or relevant.
- Run doctests, async tests, snapshot review, benchmarks, Miri, or fuzzing only when the task touches that risk area or the repository requires it.
- Confirm tests do not rely on order, global state, or external services unless explicitly marked as integration tests.

## Evidence Focus

- In the evidence summary, name the behavior verified and the Cargo commands run.
