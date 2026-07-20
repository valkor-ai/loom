# TypeScript Core Quality

## When To Use

- Load for every task that changes TypeScript application, library, API client, UI state, domain model, or shared contract code.
- Use it for baseline static correctness and for keeping TypeScript types aligned with runtime behavior.
- Do not expand a generated-file-only or non-TypeScript task just because this reference exists.

## Boundary Decisions

- Treat HTTP responses, storage records, environment variables, `JSON.parse`, user input, and third-party callbacks as `unknown` until the boundary validates or maps them.
- Keep DTOs, domain objects, form drafts, and view models separate when nullability, formatting, mutability, or lifecycle differs.
- Use a discriminated union for workflow or request states when multiple states can occur; make the transition rules explicit and exhaustive.
- Use branded or opaque IDs only when two identifiers share a primitive but must not be mixed. Construct them through a checked factory.
- Prefer `import type` for type-only dependencies and keep shared contract modules independent of framework runtime imports.

## Implementation Focus

- Preserve the repository's strictness level. Do not add `any`, broad `unknown as T`, or a compiler downgrade to make a local error disappear.
- Give exported functions, hooks, services, non-trivial component props, and public package APIs explicit parameter and return types; let obvious locals infer.
- Match optional and nullable fields to actual serialization behavior. An omitted field and an explicit `null` are different protocol states when the API says so.
- Avoid `as` in business logic. If an assertion is unavoidable at a framework boundary, keep it local and show the runtime proof beside it.
- Prefer literal unions or `as const` objects for new state and protocol discriminants unless the repository already uses enums.

## Failure Modes

- Do not silence a mismatch with a cast when the value came from a network, storage, or user boundary.
- Do not reuse a response type for an editable form when incomplete or invalid drafts are valid during interaction.
- Do not move framework types into a shared domain contract merely to avoid defining a small adapter.
- Keep compatibility adapters at the edge when an upstream payload cannot yet match the domain model.

## Verification Focus

- Run the configured typecheck or build command after source changes, and run runtime tests for changed serializers, mappers, reducers, clients, and domain functions.
- Exercise invalid or partial boundary data when static types are backed by runtime validation.
- Confirm that changed imports do not introduce a type-only/runtime cycle and that no new unchecked assertions were added.

## Evidence Focus

- Record the decision actually made: boundary validation, exported API typing, discriminated state, DTO/domain separation, branded ID, or assertion containment.
