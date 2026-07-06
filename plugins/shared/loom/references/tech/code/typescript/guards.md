# TypeScript Runtime Guard Quality

Use this topic reference when `tech/code/typescript/guards.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task consumes unknown data from APIs, form submissions, URL/query params, local or session storage, files, environment variables, web messages, or third-party libraries.
- The task adds type predicates, assertion functions, schema validators, discriminated union narrowing, or branded value construction.
- If all values are produced inside already-typed code and no boundary is crossed, do not add guards just because this reference is available.

## Implementation Focus

- Keep boundary input typed as `unknown` until it is checked. A guard should prove shape, required fields, allowed enum values, numeric ranges, and nullable/optional semantics that matter to the business flow.
- Prefer the repository's existing validation library when one is already used. Do not introduce a new schema dependency for a small local guard unless the task owns validation architecture.
- Type predicates must be pure checks. They should not normalize data, mutate objects, perform I/O, or silently fill missing business fields.
- Use assertion functions for fail-fast internal invariants and predicates or parser functions for recoverable external input. Choose the error style that matches the surrounding code.
- Validate discriminants before switching on a union. When multiple versions of a payload exist, guard the version first and map old shapes to the current domain shape explicitly.
- Construct branded IDs, money values, dates, and other constrained primitives through guards or small factories. Do not cast raw strings or numbers directly to branded types in business code.
- Keep user-facing validation messages separate from developer diagnostics when the application already has that distinction. Boundary errors should be actionable without leaking internal stack or schema details.
- If a guard transforms data, make that visible in its name and return type, such as `parseAccountResponse` instead of `isAccountResponse`.
- Do not trust generated client types or OpenAPI types for runtime safety when the data crosses a network or storage boundary; generated types describe expectations, not proof.

## Verification Focus

- Add negative tests for missing required fields, wrong primitive types, invalid discriminants, unsupported enum values, malformed dates, out-of-range numbers, and unexpected nulls where relevant.
- Add at least one positive test that proves the narrowed value can be used by the downstream code without additional assertions.
- For storage or API guards, test malformed persisted or remote data rather than only happy-path fixtures created by the same code.
- Run typecheck to confirm branches narrow without `as` assertions after the guard.

## Evidence Notes

- Record `typescript.guards` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/typescript/guards.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the protected boundary and the invalid cases covered.
