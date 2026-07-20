# Kotlin Testing Quality

## When To Use

- The task adds or changes Kotlin tests, coroutine tests, Flow assertions, Ktor route tests, Compose tests, KMP tests, mocks/fakes, or behavior implemented in Kotlin.
- Use this when Kotlin behavior needs proof through Gradle, Kotlin test, JUnit, MockK, Turbine, Ktor test, Compose test, or platform test targets.
- Follow the repository's existing test framework and source-set layout unless the task explicitly owns test infrastructure.

## Implementation Focus

- Test sealed states, validation branches, null handling, and domain transitions through public functions or screen/view-model state, not private implementation order.
- Use `runTest` for coroutine code and control virtual time for delays, debounce, timeout, and retry behavior. Do not use `runBlocking` in normal unit tests unless the repo already does for legacy reasons.
- Use Turbine or equivalent for Flow emission order, completion, cancellation, and error assertions when Flow behavior changes.
- Prefer fakes for repositories, APIs, clocks, dispatchers, and storage when behavior is small. Use MockK or mocks for external dependencies where interaction assertions matter.
- For Ktor, use application tests that exercise route registration, serialization, auth, validation, status codes, and response bodies.
- For Compose, assert visible state and interactions: loading, empty, error, input validation, enabled/disabled controls, navigation callbacks, and list rendering.
- For KMP, place tests in `commonTest` when behavior is shared and platform test source sets when actual implementations contain logic.
- Keep dispatchers, scopes, temp files, databases, servers, and background jobs cleaned up after tests.
- Avoid snapshot-only UI tests for meaningful workflow behavior unless the repository already uses snapshots and targeted assertions also cover state.

## Decision Rules

- Choose the narrowest test layer that proves the changed contract: pure domain/state tests for business rules, `runTest` for coroutine scheduling, `testApplication` for Ktor wiring, Compose tests for visible state and interaction, and platform tests for `actual` implementations.
- Keep test fixtures aligned with the repository's dependency boundary. Use fakes for owned ports and mocks only where an interaction with an external dependency is itself the contract.
- Assert failure and recovery behavior, not only the happy path, whenever the implementation introduces validation, cancellation, retry, authentication, persistence, or disabled UI states.
- Do not create a fixed coverage target solely to satisfy this reference. Report the changed branches and the evidence that covers them; add coverage thresholds only when the repository already enforces them.
- Keep virtual time, dispatchers, application engines, databases, temporary files, and background jobs isolated and cleaned up so tests do not depend on execution order.

## Verification Focus

- Run the configured Gradle test task for changed modules or the narrowest relevant target.
- Run lint/static analysis tasks such as `ktlint` or `detekt` when configured.
- For coroutine/Flow changes, verify cancellation and pending job cleanup.
- For KMP, record platform targets run and explicitly name any unavailable target.

## Evidence Focus

- In the evidence summary, name the behavior verified and the Kotlin/Gradle commands run.
