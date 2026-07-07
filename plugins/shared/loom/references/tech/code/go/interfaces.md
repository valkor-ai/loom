# Go Interface Quality

## When To Use

- The task changes Go interfaces, dependency boundaries, constructors, adapters, mocks, standard I/O integration, or package contracts.
- Use this when interface shape affects testability, package boundaries, dependency inversion, or public API clarity.
- If the task only edits a concrete private implementation, do not add an interface just because this reference is available.

## Implementation Focus

- Define interfaces at the consumer boundary when possible. A package that needs `Save(ctx, item)` should define that small need, not import a broad provider-owned interface.
- Keep interfaces small and behavior-focused. Split creator, reader, writer, lister, sender, or clock seams instead of adding a "god interface" for all methods an implementation happens to have.
- Accept interfaces and return concrete structs unless the repository has a specific factory/plugin pattern that returns interfaces.
- Use standard library interfaces before inventing new ones: `io.Reader`, `io.Writer`, `io.Closer`, `fs.FS`, `http.Handler`, `encoding.TextMarshaler`, and similar contracts.
- Add compile-time satisfaction assertions only for exported adapters, important framework contracts, or easily broken implementations. Do not add assertions for every local mock.
- Use constructor injection for required dependencies. Functional options are useful for optional configuration with defaults; do not use them to hide required dependencies.
- Avoid nil-interface traps. Be explicit about whether a dependency can be nil, and prefer constructors that validate required interfaces.
- Use type assertions and type switches sparingly at integration boundaries. For normal business variation, prefer interface methods or explicit discriminants.
- Keep mocks or fakes close to tests unless a reusable test package is already established. Production packages should not grow test-only interfaces.

## Verification Focus

- Test behavior through the interface seam when the seam is part of the change.
- Run compile/build tests that prove concrete implementations satisfy important exported interfaces.
- Confirm the interface does not include methods unused by the consuming code.
- For functional options or constructors, test defaults, supplied options, and missing required dependency behavior.

## Evidence Focus

- In the evidence summary, name the interface decision: consumer-side seam, standard interface reuse, constructor injection, functional option, compile-time assertion, or interface split.
