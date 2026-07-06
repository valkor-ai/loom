# PHP Testing Quality

Use this topic reference when `tech/code/php/testing.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. This file applies PHP testing and static-analysis guidance to task-owned changes.

## When To Use

- The task changes PHP behavior, validation, persistence, framework routes, service classes, CLI commands, async handlers, or tests.
- Use this when PHPUnit, Pest, PHPStan, Psalm, framework feature tests, mocks, fixtures, or coverage expectations affect delivery quality.
- If the task only edits non-PHP files, use this only when PHP verification is still the correct proof for the changed behavior.

## Implementation Focus

- Follow the test framework already present: PHPUnit or Pest, framework base test classes, database refresh traits, factories, fixture style, and naming conventions.
- Prefer feature/integration tests for HTTP validation, authorization, serialization, persistence, and framework wiring. Use unit tests for pure services, value objects, policies, and validators.
- Add data providers or Pest datasets for validation matrices and state-transition tables where they make branches clearer than repeated tests.
- Use test doubles at owned boundaries: external services, mailers, queues, event buses, clocks, and repositories. Do not mock the class under test or overspecify internal method calls.
- Keep database tests isolated using the repository's transaction, refresh, or container strategy. Do not make tests depend on execution order or shared mutable fixtures.
- For PHPStan/Psalm, add precise generics, array shapes, and PHPDoc where language types cannot express the contract. Do not suppress analysis findings without a task-owned reason.
- If changing error handling, include tests for exception/result shape and user-visible error contract, not only the successful branch.
- When touching queues/events/async handlers, test both dispatch/payload and handler effect if the handler owns behavior.

## Verification Focus

- Run the targeted PHP test command and the configured static-analysis command when available.
- For framework endpoints, verify successful request, validation failure, authorization failure when relevant, and persisted/read-back state.
- For service/domain code, verify edge cases, invalid states, and dependency failure behavior.
- For test-only changes, make sure the new/changed test fails for the intended reason before the implementation would satisfy it when feasible.

## Evidence Notes

- Record `php.testing` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/php/testing.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the proof type: PHPUnit/Pest unit test, framework feature test, database assertion, static analysis, data-provider matrix, mock/fake boundary, queue/event proof, or known verification gap.
