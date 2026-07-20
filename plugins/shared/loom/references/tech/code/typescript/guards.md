# TypeScript Runtime Guard Quality

## When To Use

- Load when a task consumes API responses, form input, URL parameters, storage, files, environment values, web messages, or third-party data.
- Load when the task adds predicates, assertion functions, schema validators, discriminant narrowing, or branded-value factories.
- Do not add guards to values created and kept inside already-typed code unless the task is explicitly proving an internal invariant.

## Boundary Decisions

- Keep external input typed as `unknown` until required fields, primitive types, enum values, ranges, and nullability are checked.
- Reuse the repository's validation library when one exists. A small local predicate is preferable to a new dependency for one boundary.
- Use pure type predicates for recoverable checks. Predicates must not mutate input, normalize fields, perform I/O, or silently invent business values.
- Use assertion functions for fail-fast internal invariants; use predicates or parser functions when the caller must handle invalid input.
- Validate a version or discriminant before narrowing a union, then map legacy payloads to the current domain shape explicitly.
- Construct branded IDs, money values, dates, and other constrained primitives through a guard or factory; never cast raw values directly in business code.

## Implementation Focus

- Name transforming functions as parsers, for example `parseAccountResponse`, and keep their output type distinct from the input shape.
- Keep user-facing validation messages separate from developer diagnostics and do not leak schema internals or stack details through an API response.
- Do not trust generated OpenAPI or client types as runtime proof; generated types describe an expectation, not the received bytes.
- Keep negative cases visible in the guard rather than accepting an object and relying on later property access to fail.

## Failure Modes

- Do not use truthiness as a substitute for a required-field check when valid values include `0`, `false`, or an empty string.
- Do not let a parser silently discard unknown fields when forward compatibility or auditability requires preserving them.
- Do not return a successful domain object before validating the fields that downstream authorization or persistence depends on.

## Verification Focus

- Add a positive case that proves downstream code can use the narrowed value without another assertion.
- Add relevant negative cases for missing fields, wrong primitives, invalid discriminants, unsupported enum values, malformed dates, out-of-range values, and unexpected `null`.
- For API or storage boundaries, test malformed remote or persisted data instead of only fixtures produced by the same serializer.
- Run typecheck and confirm the narrowed branches do not require `as` assertions.

## Evidence Focus

- Record the protected boundary, the parser or guard used, and the invalid input classes covered by tests.
