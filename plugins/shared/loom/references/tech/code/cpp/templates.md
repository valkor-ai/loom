# C++ Generic And Template Contracts

## When To Use

Use this reference only when the task owns a reusable template/generic contract, constraints, compile-time dispatch, CRTP/mixin, variadic API, or metaprogramming required by real consumers.

## Implementation Focus

### Start From The Consumer Contract

Identify supported type families, required operations/semantics, ownership, customization point, error behavior, performance/ABI needs, and at least two real consumers before designing the template.

Prefer a normal function/class/runtime interface when only one concrete type exists or dynamic substitution is the actual requirement.

Keep public template APIs small and implementation details in `detail` namespaces or source with explicit instantiation when possible.

### Constraints And Diagnostics

Use concepts/requires for supported standards and named constraints that express caller-visible requirements. For older standards, keep SFINAE/detection idioms localized.

Avoid unconstrained templates that fail deep inside implementation. Negative use should produce a diagnostic near the call with a meaningful missing requirement.

Do not create overlapping/ambiguous overload sets; test conversions, cv/ref qualifiers, derived types, proxies, and initializer-list interactions.

### Deduction, Forwarding, And Lifetime

Understand template deduction versus explicit types, decay, array/function handling, forwarding references, reference collapsing, and `decltype(auto)`.

Use `std::forward` only with the exact forwarding reference and only once per logical value. Do not return references/decltype(auto) to locals/temporaries or store forwarded references beyond their lifetime.

Constrain universal-reference constructors so they do not hijack copy/move or unrelated conversions.

### Variadics And Compile-Time Branching

Define empty/single/multiple pack behavior, evaluation order, and ownership for fold expressions and pack expansion. Preserve short-circuit semantics where required.

Use `if constexpr` for related type-specific branches under one coherent operation. Ensure discarded branches are still valid where non-dependent syntax requires it.

Avoid recursive metaprogramming when standard traits, folds, constexpr functions, or generated tables are clearer and cheaper to compile.

### CRTP And Customization

Use CRTP for proven static polymorphism, mixins, or compile-time customization. Prevent accidental slicing/misderived types and keep the derived contract explicit.

Prefer standard customization points, tag_invoke-like local conventions, policies, or free functions according to repository style. Do not expose inheritance just to share one helper.

Expression templates require measurable temporary elimination and strict operand lifetime/aliasing rules; they are not a default vector API pattern.

### Instantiation, ODR, And Build Cost

Templates normally require visible definitions. Keep definitions `inline`/header-safe, avoid non-inline globals/static members violating ODR, and control explicit instantiations across translation units.

Assess code size, debug symbol growth, compiler memory/time, and ABI exposure for many instantiations. Use extern/explicit instantiation or type erasure when the consumer set is bounded and build cost matters.

Public templates expose implementation and can break consumers on change; preserve semantic/version compatibility even without a traditional binary ABI.

### Compile-Time Data And Errors

Keep constexpr/type-level computations bounded and guard integer overflow, recursion depth, index bounds, and invalid packs. Runtime validation is still required for runtime input.

Use static assertions for invariants callers can act on, not to repeat a concept or leak implementation internals.

## Verification Focus

- Instantiate at least two meaningful supported types plus cv/ref/value edge cases.
- Add negative compile tests/static checks when repository infrastructure supports them.
- Test forwarding/move counts, returned lifetimes, empty packs, overload resolution, and customization behavior.
- Build all consuming targets and monitor compile time/binary size when abstraction breadth changes.
- Exercise runtime correctness and error behavior; compile success is not semantic proof.

## Evidence Focus

Name the consumer contract, constraint/customization design, lifetime/instantiation decision, supported and rejected types, and build/runtime proof. Template density or “zero cost” claims are not evidence.

## Unsafe Defaults

- Template layer added for one concrete consumer.
- Unconstrained universal references hijacking overloads.
- Forwarded/reference/view values stored past source lifetime.
- Deep trait/SFINAE errors exposed to callers.
- Expression templates introduced without benchmark/lifetime design.
- Header definitions creating ODR or code-size problems.
