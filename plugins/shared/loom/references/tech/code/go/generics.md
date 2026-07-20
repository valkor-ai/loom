# Go Generic Type Contracts

## When To Use

Use this reference only when the task explicitly owns a reusable type-parameter contract with multiple real concrete consumers. Prefer ordinary functions or behavior interfaces when type independence is not the problem.

## Implementation Focus

### Consumer Need

Start with duplicated algorithm/data-structure behavior and at least two meaningful types. Confirm generics improve type safety/readability without erasing domain operation names or adding constraints callers struggle to understand.

Do not create generic CRUD repositories, services, mappers, or containers that hide business invariants and transactions behind one shape.

Keep exported generic APIs small; internal generic helpers can remain local where they remove mechanical duplication.

### Constraints And Type Sets

Use `any` only when no operations are required. Use `comparable` only for equality/map-key/set semantics. Define small named constraints from allowed operations/types.

Use `~T` when named types with that underlying representation should participate and the algorithm is valid for all included types. Union terms are type sets, not runtime sum types.

Avoid overly broad numeric/ordered constraints when overflow, NaN, signedness, duration/money, or domain semantics differ.

Behavior belongs in interfaces with methods; type terms cannot be used as ordinary value interfaces outside constraints.

### Inference And API Shape

Arrange parameters so common calls infer type arguments naturally. Explicit type arguments are acceptable when the type is not represented in value parameters, but frequent verbose calls can signal a poor API.

Methods cannot introduce their own new type parameters independently; choose a generic receiver/type or generic free function according to ownership.

Avoid returning interface/any from a generic function and immediately type-asserting; that discards the type safety the abstraction should provide.

### Zero, Nil, And Absence

Declare zero-value semantics for generic values/containers. Use `(T, bool)`, `*T`, option/result type, or error according to absence/error ownership; returning only zero `T` can conflate valid values and missing data.

Not every `T` is nil-able, ordered, hashable, copy-cheap, immutable, or safe as a map key. Constraints and code must not assume those properties implicitly.

Avoid comparing generic values through reflection or converting to string for identity.

### Copying, Pointers, And Methods

Generic assignment/parameter passing copies values; large/mutex/resource-containing types may have unsafe/expensive copy semantics. Define pointer constraints/constructors only when actual consumers require them.

Understand method sets for `T` and `*T`; do not assume you can instantiate or call pointer receiver methods without a suitable constraint/value.

Do not use unsafe/reflection to work around a constraint that does not express the real contract.

### Performance And Code Size

Generics may be shape/dictionary/specialization implemented depending on compiler/types; do not promise zero-cost or monomorphization details without measurement.

Assess build time/binary size/escape/allocation only when performance ownership exists. A behavior interface may be simpler and fast enough.

### Compatibility

Preserve module `go` directive/toolchain and consumer support. Adding generics can raise minimum Go version and change public source compatibility.

Changing exported constraints can break or broaden callers in subtle ways; test representative downstream named types.

## Verification Focus

- Compile/test at least two meaningful concrete/named types plus zero/absence/boundary cases.
- Add compile-fail/type-check tests only if repository tooling supports stable diagnostics/contracts.
- Verify inference, named underlying types, pointer/value method sets, and rejected types.
- Run benchmarks/escape/build-size checks only for explicit performance claims.
- Build downstream consumers under the declared minimum Go version when public APIs change.

## Evidence Focus

Name consumers, constraint/zero/absence contract, inference/method-set decision, and supported/rejected type proofs. Generic syntax or one `int` test does not establish reuse or semantic safety.

## Unsafe Defaults

- Generic abstraction introduced for one type/consumer.
- `comparable`/`any` used as a vague business-object constraint.
- Generic repository erasing domain operations/invariants.
- Zero `T` returned when absence is ambiguous.
- Reflection/unsafe/type assertions bypassing constraints.
- Minimum Go/consumer compatibility raised implicitly.
