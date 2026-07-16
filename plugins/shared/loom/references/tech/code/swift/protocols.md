# Swift Protocol Quality

This file applies protocol-oriented design to Swift task changes.

## When To Use

- The task changes Swift abstractions, dependency boundaries, test doubles, generic APIs, associated types, protocol extensions, type erasure, conditional conformance, or reusable capabilities.
- Use this when a protocol can make a real domain, platform, persistence, networking, or test boundary clearer.
- If there is only one concrete type and no current variability or test boundary, do not add a protocol just for ceremony.

## Implementation Focus

- Design small capability protocols around behavior the caller needs, not around every method on a concrete type.
- Use associated types and generics when the concrete associated value matters at compile time. Use type erasure only when values with different concrete types must be stored or passed uniformly.
- Prefer protocol composition over broad umbrella protocols. Keep constraints visible at call sites so requirements remain understandable.
- Put shared default behavior in protocol extensions only when it is correct for all conformers. Do not hide stateful or surprising behavior in an extension.
- Use opaque return types (`some Protocol`) when the implementation can stay hidden and callers do not need heterogeneous storage.
- Keep dependency-injection protocols near the boundary they abstract unless the repository has a central module for shared contracts.
- Retroactive conformances for external types should be rare and local. Avoid making standard/library types conform globally when it can conflict with other modules.
- Conditional conformance is useful for collection/wrapper types, but only when the behavior truly depends on element constraints.

## Decision Rules

- Define a capability protocol around the methods the consumer needs. Do not reproduce every method on a concrete service or create a protocol when there is no substitution, platform, or test boundary.
- Use associated types/generics when the concrete type relationship matters at compile time; use type erasure only when heterogeneous values must be stored or passed uniformly.
- Prefer protocol composition and visible constraints over broad inheritance. Keep default behavior in extensions only when it is valid for every conformer and does not hide state or invariants.
- Use `some Protocol` when implementation identity can remain hidden and heterogeneous storage is not required. Use `any Protocol`/type erasure when the runtime needs a value with unknown concrete type.
- Keep dependency protocols near the boundary they abstract, and keep retroactive conformances local and justified to avoid global behavior conflicts.
- Verify conditional conformance and associated-type constraints with representative conformers, not only by compiling the protocol declaration.

## Verification Focus

- Compile all conformers and call sites after protocol changes; generic/protocol errors often surface away from the edited file.
- Test with at least one alternate implementation, fake, or mock when the protocol exists for substitution.
- For type erasure, test forwarding of success and failure behavior and ensure identity/equality semantics are intentional.
- Verify protocol extension defaults do not bypass concrete type invariants.

## Evidence Focus

- In the evidence summary, name the protocol decision: capability boundary, associated type/generic constraint, composition, default extension, type erasure, opaque return, conditional conformance, or substitution test.
