# TypeScript Implementation Pattern Quality

Use this topic reference when `tech/code/typescript/patterns.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes API clients, repositories, service layers, factories, builders, reducers, state machines, dependency wiring, mappers, or reusable TypeScript abstractions.
- Use this when a pattern choice affects maintainability or correctness. Do not introduce a pattern object or class when a direct function and type would be clearer.
- Repository conventions and existing architectural boundaries take precedence over this reference.

## Implementation Focus

- Keep pattern weight proportional to the problem. Use factories/builders for validated multi-step construction, test fixture assembly, or complex dependency setup; do not wrap simple object literals in ceremony.
- Centralize API client behavior when adding frontend or Node HTTP calls: base URL resolution, headers, response parsing, error normalization, cancellation, and typed request/response mapping should not be duplicated across screens.
- Keep repository or gateway interfaces aligned with actual persistence or remote operations. Do not expose generic CRUD methods if the business workflow needs named operations with validation and state transitions.
- For multi-step UI or workflow state, use a reducer or explicit state machine with discriminated states. Avoid scattered booleans such as `isLoading`, `isSaving`, `hasError`, and `selectedId` when combinations can become invalid.
- Keep DTO/domain/view-model mapping at boundaries. Dates, money, enum labels, IDs, and optional fields should be transformed in one place rather than repeatedly inside components or handlers.
- Use `Result`/`Either` styles only if the repository already uses them or the task owns error-handling architecture. Otherwise follow the local throw/return/null convention consistently.
- Match the project's dependency style. Do not add global mutable singletons for clients, stores, or configuration if the codebase uses dependency injection, hooks, context, or explicit parameters.
- Keep abstractions testable by injecting clocks, UUID generators, network clients, and storage only where nondeterminism affects behavior. Do not inject every pure helper.
- Avoid circular dependencies by moving shared contracts to a neutral module rather than importing upward across layers.

## Verification Focus

- Test the behavior provided by the pattern: state transitions, API error mapping, repository/gateway contract, factory validation, or mapper round-trip.
- For API clients, test success, non-2xx response, malformed payload where relevant, and cancellation or timeout if implemented.
- For reducers/state machines, test every allowed transition and at least one disallowed transition.
- Run typecheck to prove the pattern improves call-site safety without requiring broad assertions.

## Evidence Notes

- Record `typescript.patterns` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/typescript/patterns.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the pattern decision and why it was needed for this task.
