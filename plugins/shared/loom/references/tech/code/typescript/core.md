# TypeScript Core Quality

## When To Use

- The task changes TypeScript application, library, API client, UI state, domain model, or shared contract code.
- Use this for baseline TypeScript correctness: strict compatibility, exported API types, domain state modeling, and static/runtime alignment.
- If the task only edits generated files, static assets, or non-TypeScript code, do not expand scope because this reference is available.

## Implementation Focus

- Keep changed code compatible with the repository's strictness level. Do not introduce `any`, broad `unknown as T`, or strictness downgrades to make a build pass.
- Treat values from HTTP, storage, environment, user input, `JSON.parse`, and third-party callbacks as unknown until validated or transformed at the boundary.
- Give exported functions, hooks, services, components with non-trivial props, and public package APIs explicit parameter and return types. Let obvious local variables infer types.
- Model workflow, request, and UI operation states with discriminated unions when more than one state can exist. Switch on the discriminant and keep an exhaustive `never` or equivalent assert for future variants.
- Keep DTOs, domain objects, form drafts, and view models distinct when they have different nullability, formatting, or lifecycle rules. Do not reuse a server response type as editable UI state if the UI can be partially filled or invalid.
- Use branded or opaque IDs only when same-primitive identifiers can be mixed in the same flow, such as `UserId` and `OrderId`. Construct branded values through a validated factory or guard; do not brand every string in the project.
- Prefer `import type` for type-only imports and avoid creating runtime import cycles while moving shared types. Keep shared contract modules free of framework-only imports.
- Align optional and nullable fields with runtime behavior. Under `exactOptionalPropertyTypes`, do not use `undefined` as a stand-in for an omitted field unless the contract really allows it.
- Avoid `as` assertions in normal business logic. If an assertion is unavoidable at a framework or validation boundary, keep it local and make the runtime proof visible in nearby code.
- Follow existing enum conventions. For new local state or API discriminants, prefer literal unions or `as const` objects unless the repository already standardizes on TypeScript enums.

## Verification Focus

- Run the repository's configured typecheck or build command, such as `tsc --noEmit`, `tsc -p <config> --noEmit`, or the framework build that performs type checking.
- Add or update tests for changed serializers, mappers, reducers, state transitions, API clients, and domain functions; typecheck alone does not prove runtime behavior.
- Verify invalid and partial data paths when static types are backed by runtime validation.
- Confirm no new unchecked `any`, broad assertions, or type-only/runtime import mistakes were introduced in changed files.

## Evidence Focus

- In the evidence summary, name the TypeScript decision made: boundary validation, exported API typing, discriminated state, DTO/domain separation, branded ID, import boundary, or assertion containment.
