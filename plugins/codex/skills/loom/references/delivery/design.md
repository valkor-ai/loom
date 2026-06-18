# Design Discipline

Use this reference when choosing modules, interfaces, adapters, data flow, or test seams.

## Prefer existing shape

- Follow the repo's established patterns before introducing a new abstraction.
- Put changes where future maintainers and agents will naturally look.
- Keep public surfaces small; hide complexity inside the implementation.

## Choose useful seams

- A seam is useful when it makes behavior testable, isolates real variation, or improves locality.
- Do not introduce an adapter just because a future variation might exist.
- Tests should usually cross the same interface that callers use.

## Avoid shallow abstractions

- Do not wrap a simple call chain with another pass-through layer.
- Before adding a module, ask whether deleting it would spread meaningful complexity back into callers. If not, it is probably too shallow.
- Prefer a small interface with clear invariants, error modes, and lifecycle expectations.

## Record trade-offs

- Capture decisions in the candidate/result evidence when they affect later work.
- Create a separate decision doc only when the repo convention or task asks for it.
- Mention rejected options when they explain why the chosen design is safer or easier to verify.
