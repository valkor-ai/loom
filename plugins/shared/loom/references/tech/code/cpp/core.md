# C++ Ownership And Interface Delivery

## When To Use

Use this reference for task-owned C++ source/header changes. Preserve the repository's declared language standard, compiler/platform matrix, error policy, ABI/public API, build system, and local Core Guidelines conventions.

Do not adopt C++20/23 syntax because the technical baseline names C++ generally; version-specific guidance is selected separately.

## Implementation Focus

### Ownership And RAII

Make resource ownership explicit for memory, handles, files, sockets, locks, transactions, threads, GPU/device objects, mapped regions, and temporary state.

Prefer value semantics and the rule of zero. Use `std::unique_ptr` for exclusive heap ownership, `std::shared_ptr` only for demonstrated shared lifetime, `std::weak_ptr` to break observing cycles, and pointers/references/views for non-owning access with documented lifetime.

Avoid application-level raw `new`/`delete`. Isolate allocator, placement construction, C interop, or intrusive ownership and prove alignment, destruction, exception, and transfer rules.

Resource wrappers should be non-copyable or deeply copyable by contract and safely movable. A moved-from object remains valid for destruction/assignment; mark moves `noexcept` when container behavior depends on it.

### Value And View Lifetimes

Use `std::span`, `std::string_view`, iterators, and references only when the source outlives every use. Do not return views into temporaries, invalidated containers, local buffers, or strings reconstructed during conversion.

Know invalidation rules for vector growth, erase, unordered rehash, string mutation, and ranges/views. Store stable IDs/indices only when their update semantics are explicit.

Prefer returning values and rely on copy elision. Do not add `std::move` to a local return when it can inhibit NRVO.

### Interfaces And Const Correctness

Use narrow types and explicit constructors for conversions that could surprise callers. Keep nullable/optional/error distinctions visible rather than encoding several meanings in a pointer or sentinel.

Apply `const` to observable read-only operations and data access without using `const_cast` to bypass ownership. Pass by value when the function consumes/copies a small value, by reference/view for non-owning access, and by owning value/pointer for transfer.

Preserve virtual destructor and override/final requirements. Avoid calling virtual functions from constructors/destructors and avoid object slicing through by-value base parameters/containers.

### Error And Exception Safety

Follow one recoverable-error style per boundary: exceptions, expected/status/result, or error codes. Assertions/undefined behavior are not user-input or runtime failure handling.

State exception guarantees for mutating/resource operations. Build new state first or use rollback guards/transactions so failure does not leak or leave partially committed invariants.

Destructors and cleanup paths must not throw. Translate C/library/system exceptions/errors at module/API boundaries without exposing secrets or unstable implementation details.

### Headers, ODR, And ABI

Headers include what public declarations require, avoid `using namespace`, macros, anonymous namespaces, and non-inline definitions that violate the One Definition Rule.

Keep implementation dependencies in source/Pimpl where build time or ABI stability warrants it; do not add Pimpl mechanically to internal code.

For externally consumed libraries, assess layout/vtable/export visibility/calling convention/symbol/version compatibility before changing public classes, enums, templates, exceptions, or allocator ownership.

Use fixed-width integers only when width is the wire/file/hardware contract. Validate narrowing, signed/unsigned conversion, overflow, shifts, and size calculations.

### Undefined Behavior And Interop

Treat lifetime, bounds, alignment, aliasing, data races, iterator invalidation, use-after-move, signed overflow, invalid shifts, and uninitialized reads as correctness defects.

At C/OS boundaries, check every status/length/ownership contract, keep callbacks' user data alive, avoid exceptions crossing C ABI, and release resources through matching allocator/API families.

Prefer C++ casts and keep `reinterpret_cast`/`const_cast` inside reviewed low-level boundaries with invariant tests.

## Verification Focus

- Build changed targets with the exact declared standard and affected compiler/platform configuration.
- Treat new changed-file warnings as defects under repository policy.
- Test success, invalid/boundary input, ownership transfer, moved-from behavior, cleanup, and error/exception guarantees.
- Run ASan/UBSan or equivalent when lifetime, bounds, casts, alignment, interop, or low-level resources change and supported.
- Check public headers independently and ABI/API compatibility when consumers exist.

## Evidence Focus

Name the owner and lifetime, interface/error contract, exception guarantee, ABI/header decision, and focused behavior/sanitizer proof. Compilation alone does not establish resource release, view lifetime, rollback, or ABI safety.

## Unsafe Defaults

- `shared_ptr` used because ownership is unclear.
- Views/references returned without source lifetime proof.
- Mixed exception/status/null/sentinel policy for one failure class.
- Raw casts or signed/unsigned conversions used to silence diagnostics.
- Public header/layout changed without consumer/ABI analysis.
- Assertions used for external/runtime errors.
- Sanitizer absence presented as proof of no undefined behavior.
