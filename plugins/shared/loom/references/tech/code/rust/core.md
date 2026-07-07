# Rust Core Quality

## When To Use

- The task changes Rust application, library, CLI, service, systems, parser, worker, or shared crate code.
- Use this for baseline Rust correctness: ownership-first API design, module visibility, type safety, iteration, unsafe boundaries, and production panic policy.
- If the task only changes generated files, config, docs, or non-Rust code, do not expand scope because this reference is available.

## Implementation Focus

- Design APIs around ownership and borrowing before reaching for `clone`, `Arc`, or interior mutability. Accept `&str`, `&[T]`, and borrowed views where callers do not need to transfer ownership.
- Keep clones intentional. Clone small/copy-like data or owned values crossing async/thread boundaries when needed, but do not clone to silence the borrow checker without understanding lifetime ownership.
- Use module visibility deliberately: keep internals private, expose `pub(crate)` only inside crate boundaries, and avoid expanding public API surface for test convenience.
- Prefer enums and newtypes for domain states, identifiers, and constrained values. Do not pass raw strings/integers through business logic when invalid states can be modeled away.
- Use iterators and combinators when they improve clarity; use straightforward loops when branching, error handling, or mutation would make iterator chains hard to read.
- Avoid `unwrap` in production paths. Use `expect` only for true invariants with a specific message, and prefer `Result`/`Option` handling for recoverable situations.
- Keep `unsafe` out of normal task work. If unavoidable, isolate it in a small function/module, document safety invariants, and add tests or Miri/sanitizer evidence where feasible.
- Avoid global mutable state. Use explicit ownership, dependency injection, once-initialized configuration, or synchronization primitives appropriate to the runtime.
- Keep serialization/deserialization types separate from domain types when validation, defaults, versioning, or visibility differ.
- Let `cargo fmt` and clippy shape code, but do not make broad mechanical rewrites unrelated to the task.

## Verification Focus

- Run `cargo test` for the changed crate/workspace or the repository's narrower configured command.
- Run `cargo fmt --check` or `cargo fmt`, and `cargo clippy --all-targets --all-features` when configured or when code complexity warrants it.
- Add tests for domain states, parse/serialize boundaries, invalid inputs, ownership-sensitive behavior, and invariant failures touched by the task.
- Confirm no new production `unwrap`, undocumented `unsafe`, broad public visibility, or unnecessary clone-heavy workaround was introduced.

## Evidence Focus

- In the evidence summary, name the Rust decision made: borrowed API, clone justification, visibility boundary, domain enum/newtype, iterator-vs-loop choice, panic policy, unsafe boundary, or serialization/domain split.
