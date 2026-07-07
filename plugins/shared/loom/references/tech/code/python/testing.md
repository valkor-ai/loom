# Python Testing Quality

## When To Use

- The task adds or changes Python tests, pytest fixtures, mocks, async tests, integration tests, snapshot/golden data, coverage configuration, or behavior implemented in Python.
- Use this when Python behavior needs proof through the repository's test stack.
- Follow existing pytest, unittest, framework, or integration-test conventions unless the task explicitly owns test infrastructure.

## Implementation Focus

- Use pytest fixtures for reusable setup with explicit cleanup. Prefer `tmp_path`, `monkeypatch`, and fixture finalizers over global state changes.
- Use `parametrize` for validation matrices, state transitions, parser inputs, edge cases, and repeated business rules. Name cases when failures would otherwise be unclear.
- Mock external boundaries such as HTTP clients, filesystem, time, random IDs, queues, mail, databases, and cloud services. Do not mock the domain logic being tested.
- Use `AsyncMock` and async fixtures for async code. Await the behavior under test and assert awaited calls, cancellation, and cleanup when relevant.
- Assert exceptions with type and meaningful message or error attributes when callers depend on them. Avoid broad "raises Exception" assertions.
- Keep integration tests behind explicit markers or commands when they require databases, Docker, network, credentials, or slow services.
- Use snapshot or golden tests only for stable serialized/rendered output, and keep update workflow explicit. Do not snapshot broad objects when targeted assertions are clearer.
- Prefer fixture factories for domain objects that need variation. Keep factories typed enough to catch invalid test data.
- Keep coverage focused on changed behavior and meaningful error paths. Do not add coverage excludes to hide untested new code.
- Clean up environment variables, temp files, monkeypatches, event loops, background tasks, and database state after tests.

## Verification Focus

- Run the configured pytest command or the narrowest package command that covers changed Python behavior.
- Run configured type/lint/format commands when tests or source changes rely on typing or style enforcement.
- For async code, verify the test runner uses the correct async plugin and leaves no pending tasks.
- For integration tests, record the marker/command used or why it was not run.

## Evidence Focus

- In the evidence summary, name the behavior verified and the Python commands run.
