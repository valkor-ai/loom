# Rust Error Handling Quality

## When To Use

- The task changes `Result`, `Option`, custom error enums, `thiserror`, `anyhow`, error conversions, user-facing error mapping, logging, recovery, or panic behavior.
- Use this when failures cross function, crate, CLI, API, async task, or user boundary.
- If no error behavior changes, preserve the existing error style.

## Implementation Focus

- Use `Result` for recoverable failures and `Option` for absence. Convert `Option` to `Result` with a meaningful error when callers need to know why absence matters.
- Use typed errors (`thiserror` or manual `Error`) for libraries and domain boundaries where callers need to match variants. Use `anyhow` primarily for application/CLI orchestration where context matters more than matching.
- Add context as errors move upward across I/O, parsing, config, network, database, and task boundaries. Do not lose the source error unless intentionally hiding it from users.
- Avoid `String` or `&str` as public error types for non-trivial APIs. They are hard to match, convert, and test.
- Use `#[from]` conversions only when conversion is semantically correct. Do not collapse distinct business failures into one generic variant.
- Map internal errors to user-facing/API/CLI errors at the boundary. Do not leak secrets, paths, SQL, tokens, or stack-like internals into normal user output.
- Preserve cancellation or shutdown semantics in async errors; do not convert cancellation into an ordinary business failure without intent.
- Use `expect` only for internal invariants that indicate a bug, with a message naming the invariant. Avoid `unwrap` on input, I/O, parsing, config, or external data.
- Log errors at process/service boundaries, not at every propagation layer; avoid duplicate noisy logs.
- Document expected error conditions for public functions when callers need to handle them.

## Boundary Decisions

- Use `thiserror` or an equivalent typed error for library/domain boundaries where callers match variants. Use `anyhow` or contextual application errors at orchestration boundaries where preserving operation context matters more than variant matching.
- Add context when crossing I/O, parsing, configuration, network, database, and task boundaries, but do not expose paths, SQL, tokens, secrets, or internal stack details in normal user responses.
- Map internal errors to API/CLI/user-facing errors once at the boundary. Avoid logging the same failure at every propagation layer; log with operation context at the service/process boundary.
- Keep cancellation and shutdown errors distinct from ordinary business failures in async code. Cleanup may run before re-propagation, but cancellation must not be silently converted to success.
- Use `#[from]` only for semantically lossless conversions. Preserve distinct business failures and include the source error when diagnostics remain useful.
- Use `expect` only for an invariant whose violation means a programming bug, with a message that names the invariant. Do not use `unwrap` for external or recoverable data.

## Verification Focus

- Add tests for each changed error branch, including invalid input, missing data, external failure, and conversion/mapping behavior.
- Test that context is preserved where diagnostics matter and sanitized where user-facing output matters.
- Confirm no new production `unwrap` handles recoverable errors.
- For typed errors, assert variants with pattern matching instead of brittle full display strings unless display text is the public contract.

## Evidence Focus

- In the evidence summary, name the error decision: Result/Option split, typed error, anyhow context, conversion, boundary mapping, cancellation semantics, panic policy, or logging boundary.
