# TypeScript Type Modeling Quality

## When To Use

- Load only when the task owns generic APIs, reusable type helpers, mapped or conditional types, template literal domains, complex DTO variants, or public declarations.
- Ordinary interfaces, unions, and local annotations belong to `typescript.core`; do not introduce advanced type machinery for routine fields.
- The runtime contract and business invariant must be known before selecting a type-level technique.

## Decision Rules

- Start with a named interface or discriminated union. Add a constrained generic only when the same invariant is reused across real call sites.
- Use `Partial`, `Pick`, `Omit`, `Required`, and `Record` only when their semantics match the operation. A patch payload with immutable fields or coupled fields needs a named update type.
- Hide conditional and mapped types behind business names such as `UpdateOrder` or `ApiResult<T>`; do not expose dense anonymous expressions at call sites.
- Use template literal types for stable route keys, event names, feature flags, or tokens only when runtime construction validates the same shape.
- Keep recursive or distributive types bounded and local to configuration, JSON, or fixtures. Prefer explicit API and persistence types when fields are part of a durable contract.
- Use `satisfies` for route tables, configuration maps, status dictionaries, and metadata where literal values must remain narrow while the shape is checked.
- If a helper requires repeated casts, deep compiler work, or type-level debugging to use, replace it with a simpler type and a runtime check.

## Implementation Focus

- Generic constraints must express a capability such as `HasId` or `Serializable`; reject unconstrained object bags and `T extends any`.
- Keep public helper types stable and usable from emitted declarations. Internal helpers should not leak into package APIs by accident.
- Keep type definitions close to the contract they protect and avoid duplicating the same DTO shape in feature modules.

## Failure Modes

- Do not make every property optional with `Partial` when the server requires a meaningful field combination.
- Do not encode arbitrary user input as a finite template literal union without a runtime parser.
- Do not accept a recursive type that slows every editor operation when an explicit bounded shape is sufficient.
- Keep generated contract types and hand-written domain types separated when their release cadence differs.

## Verification Focus

- Run the package typecheck and test representative valid and invalid usages of every public helper.
- When declarations are emitted, inspect that `.d.ts` output exposes usable names and no private path or helper implementation.
- Watch typecheck time and editor responsiveness after adding recursive, distributive, or very large union types.

## Evidence Focus

- Record the modeling choice and the invariant it protects: constrained generic, utility DTO, discriminated result, template key, config map, recursive type, or declaration shape.
