# TypeScript Type Modeling Quality

Use this topic reference when `tech/code/typescript/types.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes generic APIs, reusable type helpers, mapped or conditional types, template literal types, route/event key types, package-level contracts, or complex DTO variants.
- Use this only when advanced typing protects a real contract or removes meaningful duplication. Ordinary business code should stay readable with direct interfaces and unions.
- If the task does not need reusable type modeling, prefer `typescript.core` guidance and avoid adding type-level machinery.

## Implementation Focus

- Start from the runtime contract and business invariant, then choose the smallest type construct that protects it. Do not add conditional, recursive, or highly generic types only to make the code look advanced.
- Generic constraints must name the capability being required, such as `HasId`, `Serializable`, or `RequestShape`. Avoid `T extends any`, unconstrained object bags, and call sites that require readers to mentally execute the type system.
- Use built-in utility types when they match the business rule. Do not use `Partial<T>` for update or draft payloads if some fields are immutable, required together, or mutually exclusive; define a named type that expresses the rule.
- Hide conditional and mapped types behind named aliases with business meaning. A call site should read as `UpdatePurchaseRequest` or `ApiResult<T>`, not as a dense stack of anonymous utility expressions.
- Use template literal types for stable domains such as route params, event names, feature flags, or CSS tokens only when runtime generation follows the same pattern. Do not type arbitrary user strings as a finite template domain.
- Keep recursive and deep utility types limited to configuration, JSON, immutable fixtures, or well-bounded nested data. Prefer explicit interfaces for API and database records where fields are part of the contract.
- Use `satisfies` for config maps, route tables, status dictionaries, and metadata objects when you need literal values preserved while checking the object shape.
- Avoid compile-time cleverness that requires casts at runtime. If a type helper cannot be used without `as`, the helper is probably too ambitious for the task.
- For exported package APIs, keep helper types stable and documented by usage. Internal type helpers can be narrower and should not leak into public declarations unless they are part of the API.

## Verification Focus

- Run typecheck after changing generic or utility types; advanced type errors often surface in downstream call sites rather than the edited file.
- For public type helpers, keep at least one representative usage in tests, examples, or production code that proves the intended inference and invalid case.
- If declaration files are emitted, run the library build and inspect that public `.d.ts` output does not expose private helper names or unusable generic signatures.
- Watch for typecheck performance regressions when adding recursive, distributive conditional, or large union types.

## Evidence Notes

- Record `typescript.types` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/typescript/types.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the modeling choice: utility DTO, constrained generic, discriminated result, template key, config map with `satisfies`, recursive type, or public declaration shape.
