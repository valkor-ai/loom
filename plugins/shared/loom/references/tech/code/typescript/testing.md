# TypeScript Testing Quality

## When To Use

- Load only when the task owns TypeScript tests, fixtures, test utilities, typecheck scripts, generated contract checks, or runtime behavior that must be proven in TypeScript.
- Typechecking is not a substitute for runtime tests of HTTP handling, validation, reducers, UI states, or persistence behavior.
- Follow the repository's existing runner and test environment unless the task explicitly owns test infrastructure.

## Decision Rules

- Test through stable public boundaries: exported functions, service methods, hooks, reducers, components, API clients, or domain operations.
- Keep fixtures typed as real DTOs or domain objects. Do not use `as any` to bypass the contract under test.
- Mock external or slow boundaries such as network, filesystem, timers, browser storage, and third-party services; keep domain logic real.
- For component tests, assert visible outcomes and user-dependent state: loading, empty, error, disabled, submitted, and success. Snapshot-only coverage is insufficient for interactions.
- For async tests, await the operation or return its promise. Restore fake timers and leave no floating promises or unhandled rejections.
- Add negative cases for guards, malformed API or storage data, invalid transitions, unsupported discriminants, permission blocks, and status rules when those paths changed.

## Implementation Focus

- Pair the configured typecheck with focused runtime tests for changed public behavior.
- Keep test helpers and fixtures in the same contract vocabulary as production code so they do not normalize away the failure being tested.
- Use the configured DOM or browser environment for browser-facing code; Node, jsdom, happy-dom, and real browser checks prove different things.
- Respect existing coverage thresholds when present. Do not exclude changed files to make a threshold pass.

## Failure Modes

- Do not treat a passing typecheck as proof that a server returned valid JSON or that a component renders the required state.
- Do not make snapshots the only evidence for an interactive flow whose loading, error, disabled, or success behavior changed.
- Do not leave a test green by mocking the domain rule or by sharing mutable fixture objects between cases.
- Keep setup deterministic so a failure identifies the changed boundary rather than test order.

## Verification Focus

- Run the changed package's test command and its configured typecheck or build command.
- When a test uses fake timers, prove that timers are advanced and restored; when it uses a browser API, verify that the selected environment supports the exercised path.
- Record an untested gap only when it is outside the task boundary or blocked by missing infrastructure.

## Evidence Focus

- Record the behavior verified, the invalid path covered, and the exact focused commands run.
