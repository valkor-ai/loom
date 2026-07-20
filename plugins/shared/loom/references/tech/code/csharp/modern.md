# C# Language-Version Features

## When To Use

Use this reference only when the task explicitly owns C# language-version APIs or changes `LangVersion`/target framework behavior. Do not modernize stable code solely because the SDK can compile newer syntax.

## Implementation Focus

### Compatibility Boundary

Confirm SDK, target framework, `LangVersion`, compiler/analyzer, runtime/BCL, source generators, test/build agents, consumers, and deployment image support. C# syntax and .NET runtime/library features have separate compatibility.

Prefer the project's SDK/target defaults unless an explicit version is needed. A language upgrade includes project/CI/tooling/consumer changes and fallback/migration evidence.

Do not use preview features without accepted preview policy and pinned toolchains.

### Records, Required Members, And Primary Constructors

Use record/value semantics when equality/deconstruction/immutability fit. Review arrays/collections, mutable properties, EF/proxy/serializer behavior, and inherited equality before selecting a record.

`required` communicates object-initialization obligations but does not validate deserialized/external values. Keep constructors/factories/validators responsible for runtime invariants.

Primary constructor parameters are parameters, not automatically fields/properties. Avoid capturing mutable/disposable dependencies ambiguously or making API compatibility less clear.

### Pattern Matching

Use property/list/relational/type patterns and switch expressions when they make closed decision logic exhaustive and readable.

Handle null, guards, overlapping cases, evaluation order, and future enum/derived types. Do not hide side effects or expensive calls in patterns/guards.

For domain states, prefer a typed closed model and exhaustive matching over strings and default branches that silently accept unknown cases.

### Collection Expressions And Spreads

Use collection expressions when target typing and allocation semantics are clear. Know whether the target is array/list/span/immutable/interface and whether spread enumerates once or allocates/copies.

Do not use them where overload resolution, builder behavior, lazy enumeration, or ownership becomes less obvious. Preserve repository style for public examples/consumers on older compilers.

### Ref Structs, Spans, And Scoped Lifetimes

Use `Span<T>`, `ReadOnlySpan<T>`, `ref struct`, `scoped`, `ref` returns, and stack allocation only inside proven synchronous lifetime boundaries.

Ref-like values cannot cross await/yield, ordinary boxing/interface/heap capture, or outlive backing storage. Avoid returning spans over stack/local/pooled buffers or using `Memory<T>` without explicit owner lifetime.

Keep unsafe/ref features isolated and paired with bounds/lifetime tests and performance evidence.

### Generic Math And Static Abstract Members

Use generic math/static abstract interface members for real numeric/generic consumers with clear constraints and supported target frameworks.

Define overflow, checked context, conversion, floating NaN/infinity, and semantic operation expectations. Avoid creating a generic layer for one numeric type.

### Interpolated Strings And Raw Literals

Raw strings improve embedded text readability but do not make SQL/HTML/JSON/shell content safe. Use parameterization/encoding/serialization rather than interpolation for untrusted data.

Custom interpolated-string handlers are advanced performance/API features requiring caller semantics, conditional evaluation, and benchmark justification.

### Source Generation, AOT, And Trimming

Use source-generated JSON/regex/logging/DI or custom generators when target runtime, reflection/trimming/startup/build needs justify them and repository tooling supports generated output.

Generators need deterministic incremental inputs, diagnostics, namespace/collision policy, analyzer package behavior, and consumer/build tests. Generated source is not a place to hide business logic.

Native AOT/trimming changes require reflection/dynamic-loading/serialization/plugin compatibility and publish-time evidence, not only normal `dotnet build`.

## Verification Focus

- Build with exact SDK/TFM/LangVersion and affected consumer/tooling matrix.
- Test record equality, required runtime validation, pattern exhaustiveness, collection allocation/overload, and span/ref lifetime behavior when used.
- Run publish/trimming/AOT and source-generator consumer tests when owned.
- Verify fallback/compatibility for multi-targeted projects and older consumers.
- Measure any performance-motivated language feature against clear code.

## Evidence Focus

Name the feature, SDK/TFM/language support, runtime/lifetime/serialization semantics, compatibility boundary, and focused behavior/publish proof. New syntax compiling locally is not production compatibility evidence.

## Unsafe Defaults

- C# 12/preview syntax introduced without declared toolchain ownership.
- Records selected despite mutable identity or incompatible serializer/ORM semantics.
- `required` treated as runtime input validation.
- Spans/ref structs escaping backing lifetime or async boundary.
- Source generation/AOT claimed complete from ordinary build only.
- Raw/interpolated strings used as injection protection.
