# C++ Language-Version Features

## When To Use

Use this reference only when the task explicitly owns APIs/features from a declared C++17/20/23 standard or changes the language target. Stable code should not be rewritten for novelty.

## Implementation Focus

### Compatibility First

Confirm target compile features, compiler versions, standard library implementation, platform SDK, deployment image, dependency compatibility, and feature-test macros. Language parser support does not guarantee the library or build system supports a feature.

Prefer `target_compile_features(... cxx_std_XX)` or repository equivalent over source assumptions. Keep fallback/guard behavior when the supported matrix spans feature availability.

Do not upgrade the standard inside source only; update build declarations, CI/toolchain presets, package consumers, docs/examples, and compatibility tests within task scope.

### Concepts And Constraints

Use standard concepts and small named domain concepts to express operations/relationships required by a generic API. Require semantics callers can understand; a syntactic expression alone may be too weak.

Put constraints where overload resolution can use them and avoid redundant `static_assert` diagnostics. Verify invalid types fail for the intended reason without making ordinary diagnostics unreadable.

Concepts do not justify templating non-generic behavior or exposing implementation details in public headers.

### Ranges And Views

Use range algorithms/pipelines when they clarify ownership and transformation. Views are lazy and often non-owning: ensure the source, captured references, and temporary adaptors outlive iteration.

Materialize when data crosses an async/storage/API boundary, must own results, or repeated traversal has different cost/semantics. Beware single-pass ranges, dangling borrowed ranges, proxy references, and mutation invalidation.

Measure complex pipelines in hot paths and preserve clear error/empty behavior.

### Expected, Optional, And Variants

Use `std::expected` when supported for recoverable value-or-error results that callers must inspect. Define a stable error type and avoid nested expected/optional/variant structures that obscure state.

Use `optional` for presence/absence, not for detailed failure. Use variants for a closed set of meaningful alternatives and make visitation exhaustive.

Preserve repository exception/status conventions at boundaries instead of mixing styles because a new type exists.

### Coroutines

Use coroutines only with an established task/runtime abstraction. The coroutine return type owns handle lifetime, scheduler/executor affinity, cancellation, exception propagation, destruction, and continuation behavior.

Never hand-roll a sample coroutine type for production without proving final suspend, frame destruction, move/copy behavior, abandonment, concurrent resume, and error/cancellation semantics.

Avoid holding locks or unsafe references across suspension and make source/owner lifetimes explicit.

### Comparison, Initialization, And Compile Time

Defaulted `<=>` is appropriate only when memberwise equality/order matches the domain. Partial/weak/strong ordering and floating-point NaN require deliberate semantics.

Designated initializers apply to aggregates and order rules; do not make public aggregate layout an accidental API. Use builders/constructors when validation or compatibility matters.

Use `constexpr`/`consteval` for stable pure computation with meaningful compile-time benefit. Keep diagnostics/build cost bounded and verify runtime-equivalent behavior where relevant.

### Formatting, Modules, And Library Availability

Use `std::format`/`print` only when the target standard library supports required formatters/locales; otherwise preserve the accepted formatting/logging library.

Modules require compiler, generator, dependency scanner, cache, test, package, and IDE support. Introduce them only as an owned build migration, not a file-local cleanup.

Feature-test macros and isolated adapters are preferable to scattered compiler-version preprocessor branches.

## Verification Focus

- Build the exact compiler/standard-library/platform matrix affected by the feature.
- Test concept acceptance/rejection, range lifetime/materialization, expected/variant states, and comparison semantics.
- Exercise coroutine completion, failure, cancellation, abandonment, and destruction on the actual runtime.
- Verify feature guards/fallbacks and package consumers when the supported matrix is mixed.
- Measure compile/runtime cost when the feature is selected for performance.

## Evidence Focus

Name the feature, accepted standard/toolchain proof, ownership/lifetime/error semantics, fallback, and behavior/compatibility assertion. A successful local compile is not cross-toolchain or runtime-lifecycle evidence.

## Unsafe Defaults

- C++20/23 feature selected from prose without build/toolchain ownership.
- Lazy view escaping its source lifetime.
- Concepts added to ordinary non-generic code.
- Coroutine handle/runtime implemented from a tutorial sample.
- Defaulted comparison used for business-specific or floating order.
- Modules introduced without build/package ecosystem support.
