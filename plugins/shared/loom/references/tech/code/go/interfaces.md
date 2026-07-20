# Go Consumer Interfaces And Adapters

## When To Use

Use this reference only when the task explicitly owns a dependency seam, interface/protocol contract, adapter, constructor, functional options, or testable boundary. Do not create an interface only because one implementation exists.

## Implementation Focus

### Define At The Consumer

Define the smallest interface in the package that consumes behavior when practical. The provider may expose a concrete type; consumers describe only methods they need.

Name interfaces by behavior/capability (`Reader`, `Store`, `Clock`, `Sender`) and avoid broad `Service`/repository interfaces mirroring every concrete method.

Accept interfaces and return concrete types by default so callers retain capabilities without forcing mocks/factories. Return interfaces only for intentional hidden implementations/plugins or established APIs.

### Standard Interfaces

Reuse `io.Reader`, `io.Writer`, `io.Closer`, `fs.FS`, `http.Handler`, `error`, encoding interfaces, `sort.Interface`, or repository-standard contracts before inventing equivalents.

Compose small standard/local interfaces only when the consumer truly needs the combined capability. Embedding can accidentally expand API compatibility requirements.

Honor semantic contracts beyond method signatures: short writes/reads, EOF, Close idempotency, context cancellation, ownership, and concurrency safety.

### Method Sets And Satisfaction

Choose pointer/value receivers deliberately because method sets determine which type satisfies an interface. Avoid changing receiver kind on published types without consumer compatibility review.

Compile-time satisfaction assertions are useful for important exported adapters/framework contracts and can document intended implementation; do not add boilerplate for every private fake.

Type assertions/switches belong at dynamic integration/plugin/serialization boundaries with checked failure. Normal domain behavior usually belongs in interface methods or explicit variants.

### Nil Interface Safety

An interface containing a typed nil pointer is non-nil. Constructors should reject missing dependencies, and methods should not rely on `if dep == nil` after accepting arbitrary implementations.

Avoid returning typed-nil concrete pointers as errors/interfaces. Test nil/typed-nil behavior when an adapter can produce it.

Do not use pointer-to-interface; interfaces already carry dynamic type/value.

### Constructors And Options

Constructors accept required dependencies/config and validate them. Return an error when configuration can be invalid; avoid partially usable objects.

Functional options fit many optional settings with stable defaults and composability. They must not hide required dependencies, order-sensitive conflicts, mutable shared config, or validation.

For simple few options, an explicit config struct is clearer. Copy caller-owned slices/maps or document retention/immutability.

### Adapters And Boundaries

Adapters translate provider types/errors/lifecycle into consumer contracts. Keep provider clients and DTOs from leaking into domain packages.

Preserve context, cancellation, idempotency, transactions, retries, and resource ownership. Do not make a thin interface that hides critical semantics callers need.

Keep observability/logging at suitable adapter/application boundaries without wrapping every method solely to log.

### Testing Seams

Prefer small hand-written fakes/stubs for narrow interfaces. Generate mocks only with repository tooling and keep generated files/version commands owned.

Do not add production interfaces solely to mock internal pure logic. Test concrete behavior directly when substitution is not needed.

## Verification Focus

- Compile important implementations against the consumer interface.
- Test constructor required/default/invalid options and typed-nil traps.
- Exercise adapter translation of success, typed/provider errors, cancellation, partial results, and cleanup.
- Verify only consumer-used methods remain and dependency direction avoids cycles.
- Run downstream consumer builds when exported method sets/interfaces change.

## Evidence Focus

Name consumer, required behavior/semantics, concrete adapters, constructor/options policy, and consumer/adapter tests. Interface existence or generated mocks do not establish a useful abstraction.

## Unsafe Defaults

- Interface created beside provider to mirror one concrete type.
- God interface combining unrelated capabilities.
- Pointer-to-interface or typed-nil dependency accepted silently.
- Functional options hiding required dependencies or invalid combinations.
- Provider types/errors/lifecycle leaking through adapter contract.
- Production abstraction added only to satisfy a mocking tool.
