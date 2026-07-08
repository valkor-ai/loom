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

## Verification Focus

- Compile all conformers and call sites after protocol changes; generic/protocol errors often surface away from the edited file.
- Test with at least one alternate implementation, fake, or mock when the protocol exists for substitution.
- For type erasure, test forwarding of success and failure behavior and ensure identity/equality semantics are intentional.
- Verify protocol extension defaults do not bypass concrete type invariants.

## Evidence Focus

- In the evidence summary, name the protocol decision: capability boundary, associated type/generic constraint, composition, default extension, type erasure, opaque return, conditional conformance, or substitution test.
