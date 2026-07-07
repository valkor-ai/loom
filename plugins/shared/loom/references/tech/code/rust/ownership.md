# Rust Ownership Quality

## When To Use

- The task changes lifetime relationships, borrowed/owned APIs, smart pointer choice, interior mutability, RAII/drop cleanup, `Cow`, `Pin`, shared state, or ownership-sensitive data structures.
- Use this when borrow checking, lifetime design, or resource ownership affects correctness or maintainability.
- If ownership is unaffected by the task, do not redesign APIs just because this reference is available.

## Implementation Focus

- Make ownership transfer explicit in function signatures. Use owned values when the callee must store or consume them; use references/slices when it only reads.
- Keep lifetime annotations minimal and meaningful. Do not add named lifetimes where elision is clear; do document relationships when returned references depend on inputs.
- Choose pointer types by ownership model: `Box` for heap-owned single values, `Rc` for single-thread shared ownership, `Arc` for cross-thread shared ownership, and references for non-owning access.
- Use `RefCell`, `Cell`, `Mutex`, or `RwLock` only when mutation through shared ownership is truly needed. Interior mutability is a design decision, not a borrow-checker escape hatch.
- Prefer `Cow` for APIs that usually borrow but sometimes normalize/own. Do not use `Cow` if the function always allocates or always borrows.
- Use RAII and `Drop` for cleanup that must always occur, but keep `Drop` implementations simple and non-panicking.
- Be careful with `Arc<Mutex<T>>`: keep lock scope short, avoid calling user code while locked, and do not hold async-incompatible locks across await points.
- Use `Pin` only for self-referential or async/future requirements. Do not introduce manual pinning unless the type's move invariants require it.
- Do not leak memory intentionally (`Box::leak`, `mem::forget`) unless process-lifetime ownership is explicitly part of the design and documented.
- Keep builders and state machines ownership-aware: consuming builders are good for required fields; mutable builders are fine when repository convention prefers them.

## Verification Focus

- Let compilation prove basic borrow/lifetime correctness, then add runtime tests for ownership-sensitive behavior such as cleanup, shared mutation, builder consumption, or dropped resources.
- Run tests that exercise error paths and early returns to prove RAII cleanup occurs.
- Use clippy to catch needless clones, deref issues, and lock/ownership smells when available.
- If unsafe, pinning, or custom drop behavior changed, add focused tests and record any Miri/sanitizer check that was run.

## Evidence Focus

- In the evidence summary, name the ownership decision: borrowed API, lifetime relationship, pointer type, interior mutability, Cow, RAII/Drop, lock scope, Pin, leak avoidance, or builder ownership.
