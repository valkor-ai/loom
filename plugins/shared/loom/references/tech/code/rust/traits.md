# Rust Trait Quality

## When To Use

- The task changes traits, generic bounds, associated types, trait objects, derive macros, conversion traits, extension traits, marker traits, or trait-based dependency boundaries.
- Use this when trait design affects public API clarity, dispatch, test seams, or compile-time guarantees.
- If a concrete type or simple function is enough, do not add a trait layer.

## Implementation Focus

- Keep traits small and capability-focused. Avoid broad traits that mirror one concrete type's full method set.
- Use associated types when each implementation has one natural related type; use generic parameters when callers may combine multiple concrete types in one implementation.
- Prefer static dispatch for hot generic code and trait objects for heterogeneous collections, plugin boundaries, or runtime flexibility. Make this choice explicit when it affects API shape.
- Ensure traits intended as objects are object-safe. Do not add generic methods or by-value `self` methods to traits that need `dyn Trait`.
- Use standard traits (`From`, `TryFrom`, `AsRef`, `Borrow`, `Iterator`, `Display`, `Debug`, `Error`, `Serialize`) when semantics match; do not invent parallel conversion/display APIs.
- Derive standard traits when behavior is purely structural. Implement manually only when semantics differ from field-by-field behavior.
- Use sealed traits when external implementations would violate invariants or make future evolution unsafe.
- Keep extension traits scoped and named to avoid surprising method pollution. Do not add extension traits for one local call site.
- Use marker traits only for real compile-time guarantees, and document safety/invariant requirements for unsafe marker traits.
- Be mindful of coherence/orphan rules when designing impls across crate boundaries; do not paint the crate into an API corner with overly broad blanket impls.

## Verification Focus

- Build examples or tests that use the trait through the intended boundary: generic static dispatch, trait object, conversion, or extension method.
- Test at least two meaningful implementors when the trait is meant to abstract multiple implementations.
- Confirm object-safety when using `dyn Trait`.
- For public traits, include documentation or examples that show required invariants and expected implementor behavior.

## Evidence Focus

- In the evidence summary, name the trait decision: associated type, generic bound, trait object, derive/manual impl, conversion trait, sealed trait, extension trait, marker trait, or coherence boundary.
