# C++ Modern Feature Quality

## When To Use

- The task changes code that may benefit from C++17/20/23 features such as concepts, ranges, coroutines, modules, spaceship comparisons, designated initializers, `constexpr`, `std::format`, or `std::expected`.
- Use this to choose modern features deliberately, not to rewrite stable code for novelty.
- If the project standard or compiler does not support a feature, do not introduce it.

## Implementation Focus

- Check compiler, standard library, and build target support before using a modern feature. Some C++20/23 library features lag behind language support.
- Use concepts to make template constraints readable and diagnostics better. Do not add concepts around ordinary non-template code.
- Use ranges/views for readable pipelines over existing ranges, but be explicit when a lazy view outlives source data or when materialization is required.
- Use coroutines only when the surrounding project has coroutine runtime/support and ownership conventions. A hand-rolled coroutine abstraction is a high-risk design decision.
- Default three-way comparison is useful for value types where all members define desired ordering. Do not generate ordering for domain types with partial or business-specific order.
- Use designated initializers for simple aggregate configuration only when the project standard supports them and field order/naming is stable.
- Use `constexpr` or compile-time computation for stable constants, lookup tables, and type-level constraints. Do not move runtime business logic into compile time without benefit.
- Use `std::format` only when available in the target standard library; otherwise follow existing formatting/logging dependency.
- Avoid C++ modules unless the project already has module build support. Modules affect build tooling and should not be introduced as a local refactor.
- Keep modern feature usage understandable to maintainers; prefer clear ordinary code when the feature does not reduce real complexity.

## Verification Focus

- Build with the exact project standard and compilers targeted by the repository.
- Add compile-time or runtime tests that prove concept constraints, range lifetime/materialization, comparison semantics, or formatting output where changed.
- Confirm no feature requires a newer standard/library than declared in build files.
- Record any platform/compiler limitation when a modern feature is guarded or avoided.

## Evidence Focus

- In the evidence summary, name the feature decision: concepts, ranges, coroutine support, comparison, designated initializer, constexpr, formatting, modules, or compatibility guard.
