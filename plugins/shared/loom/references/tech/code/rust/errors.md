# Rust Error Handling Quality

Use this topic reference when `tech/code/rust/errors.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

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

## Verification Focus

- Add tests for each changed error branch, including invalid input, missing data, external failure, and conversion/mapping behavior.
- Test that context is preserved where diagnostics matter and sanitized where user-facing output matters.
- Confirm no new production `unwrap` handles recoverable errors.
- For typed errors, assert variants with pattern matching instead of brittle full display strings unless display text is the public contract.

## Evidence Notes

- Record `rust.errors` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/rust/errors.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the error decision: Result/Option split, typed error, anyhow context, conversion, boundary mapping, cancellation semantics, panic policy, or logging boundary.
