# TypeScript Testing Quality

Use this topic reference when `tech/code/typescript/testing.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task adds or changes TypeScript tests, fixtures, test utilities, component/service tests, typecheck scripts, generated contract checks, or behavior covered by typed boundaries.
- Use this when the implementation has runtime behavior in TypeScript. Do not rely on static typechecking as the only proof for user-visible or business behavior.
- Follow the existing test runner and repository test style unless the task explicitly owns test infrastructure.

## Implementation Focus

- Pair typecheck with runtime tests for changed public behavior. TypeScript can prove call shapes, but it cannot prove HTTP response handling, reducer behavior, validation messages, or storage migration logic.
- Keep fixtures typed as real DTOs or domain objects. Avoid `as any` fixtures that bypass the same contract the code is supposed to honor.
- Include negative cases for guards, invalid states, malformed API/storage data, permission or status blocks, and unsupported enum/discriminant values when those paths changed.
- Test through stable boundaries: public functions, service methods, hooks, reducers, components, or API clients. Avoid asserting private helper calls unless the helper is the unit being delivered.
- Mock only external or slow boundaries such as network, filesystem, timers, browser storage, and third-party services. Do not mock the domain logic under test.
- For component tests, assert visible outcomes and state changes the user depends on: loading, empty, error, disabled, submitted, and success states. Avoid snapshot-only tests for interactive behavior.
- For async TypeScript tests, await the operation or return the promise. Do not leave floating promises, unflushed timers, or unhandled rejections.
- If the repository uses coverage thresholds, add meaningful cases rather than excluding changed files.

## Verification Focus

- Run the configured test command for the changed package and the configured typecheck/build command.
- When tests use fake timers, advance and restore timers explicitly and prove no pending timers remain if the runner exposes that check.
- When tests use DOM or browser APIs, confirm the configured environment matches the code path: Node, jsdom, happy-dom, browser, or framework runner.
- Record any untested gap only when it is outside the task boundary or blocked by missing infrastructure.

## Evidence Notes

- Record `typescript.testing` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/typescript/testing.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the behavior verified and the commands run.
