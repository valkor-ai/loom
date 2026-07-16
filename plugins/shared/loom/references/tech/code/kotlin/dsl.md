# Kotlin DSL And Generic Design Quality

## When To Use

- The task introduces or changes a type-safe builder, lambda with receiver, delegated property, inline/reified helper, operator overload, or generic abstraction in Kotlin.
- Use this when a fluent API or generic boundary is part of the owned implementation. Do not load it for ordinary Kotlin classes that do not define a reusable abstraction.

## Implementation Focus

- Design the DSL around a small owned model and make invalid intermediate states difficult to express. Validate the final model at the boundary before it reaches persistence, routing, build configuration, or another side effect.
- Use a lambda with receiver when the block configures one coherent owner. Keep nested receivers shallow, name ambiguous receivers explicitly, and avoid implicit calls that make it unclear which object is being mutated.
- Prefer `@DslMarker` when nested builders expose overlapping members. This prevents a child block from accidentally configuring a parent object.
- Use scope functions for one purpose at a time. `apply`/`also` should not hide validation or I/O; use named functions when a chain contains business decisions or more than one side effect.
- Keep extension functions and operator overloads unsurprising and domain-owned. Do not redefine common operators to perform I/O, mutate hidden global state, or make control flow difficult to read.
- Use `inline` and `reified` only when they remove a real type-token or allocation boundary. Avoid exposing implementation-specific generic constraints through public APIs without a compatibility reason.
- Use delegated properties when the delegate owns observable semantics such as lazy initialization, configuration lookup, or state persistence. Do not use delegation to hide a simple field or lifecycle that should be explicit.
- Prefer sealed hierarchies and constrained type parameters when they encode a finite protocol. Keep variance and nullable bounds explicit at the public boundary.

## Verification Focus

- Test builder defaults, required fields, nested-scope restrictions, invalid combinations, generic type selection, and delegation lifecycle behavior through the public DSL.
- Compile or run the narrowest module target that consumes the abstraction. For a published or shared API, verify source compatibility and representative call sites.
- Check that evaluation order, receiver ownership, exceptions, and side effects remain visible in tests. Do not use a sample that only proves the fluent syntax compiles.

## Evidence Focus

- In the evidence summary, name the abstraction decision: builder state model, receiver scope, DSL marker, extension/operator boundary, delegation lifecycle, inline/reified use, or generic variance.
- Record the invalid-state or compatibility case that was verified, not only the command that compiled the happy path.

## Failure Modes

- Do not add a DSL because a builder would be shorter to type. A regular constructor or named function is preferable when it is clearer.
- Do not copy an external DSL sample without adapting receiver ownership, validation, naming, and lifecycle to the repository's existing API.
