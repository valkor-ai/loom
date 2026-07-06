# C++ Template Quality

Use this topic reference when `tech/code/cpp/templates.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes templates, variadic templates, concepts/constraints, type traits, CRTP, SFINAE, expression templates, compile-time computation, or generic library APIs.
- Use this only when compile-time polymorphism removes real duplication, improves type safety, or is required by a public generic API.
- If a direct function/class or runtime interface is clearer, do not add a template layer.

## Implementation Focus

- Prefer concepts or `requires` clauses for C++20+ constraints. Use SFINAE only when the project standard requires it or existing code already uses it.
- Keep template diagnostics readable. Named constraints and small helper aliases are better than deeply nested `enable_if` or type trait expressions.
- Use `if constexpr` for type-specific branches where all cases share one coherent operation. Do not hide unrelated behavior inside a single generic function.
- Use variadic templates and fold expressions for real variable-arity APIs. Validate empty-pack behavior explicitly.
- Use CRTP only for static polymorphism or mixins that avoid virtual dispatch and share meaningful behavior. Do not use it where a normal base class or free function is clearer.
- Keep expression templates limited to numeric/vector/domain libraries where temporary elimination is measurable and maintainable.
- Avoid exposing implementation-heavy template internals in public headers unless the library is intentionally header-only or generic.
- Be careful with compile-time computation costs. A fast runtime function may be preferable to template metaprogramming that slows builds and confuses diagnostics.
- Keep ABI and code bloat in mind for exported generic APIs. Avoid instantiating large templates for many types without benefit.
- Document constraints and invariants for public templates so callers understand supported types and failure modes.

## Verification Focus

- Build all affected template instantiations under the project standard.
- Add compile/runtime tests for at least two meaningful supported types when a generic API claims type independence.
- Add negative constraint tests when the repository has compile-fail or static assertion test infrastructure.
- Check compile errors remain understandable for invalid use, especially for public templates.

## Evidence Notes

- Record `cpp.templates` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/cpp/templates.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the template decision: concept constraint, variadic API, if constexpr branch, CRTP mixin, expression template, type trait, compile-time computation, or code-bloat containment.
