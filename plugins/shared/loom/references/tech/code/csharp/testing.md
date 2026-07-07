# C# Testing Quality

## When To Use

- The task adds or changes C# tests, xUnit/NUnit/MSTest setup, WebApplicationFactory tests, EF tests, Blazor component tests, mocks/fakes, async/cancellation tests, or behavior implemented in C#.
- Use this when C# behavior needs proof through the .NET test stack.
- Follow the repository's existing test framework and naming style unless the task explicitly owns test infrastructure.

## Implementation Focus

- Test externally visible behavior through services, endpoints, handlers, repositories, components, or public methods. Avoid tests that only lock in private implementation order.
- Use parameterized tests for validation matrices, status transitions, boundary values, and repeated business rules.
- Mock external boundaries such as HTTP clients, clocks, queues, mail, filesystem, identity providers, and payment/cloud services. Prefer fakes for small stateful dependencies.
- For ASP.NET Core, use `WebApplicationFactory` or `TestServer` to prove routing, DI, filters, auth, validation, status codes, and response bodies when HTTP behavior changes.
- For EF, prefer provider-compatible integration tests for queries and migrations. Avoid using in-memory provider as proof for relational behavior unless the app already accepts that limitation.
- For async code, await operations, pass test cancellation tokens where relevant, and assert cancellation/error propagation. Do not leave fire-and-forget work unobserved in tests.
- For Blazor, assert rendered states and user interactions: loading, empty, validation, submit, error, auth, and disposal paths.
- Keep test data builders focused on domain meaning. Do not create huge object mothers that hide required fields or invalid combinations.
- Assert wrapped errors with typed results, `ProblemDetails`, or exception types according to the app contract; avoid brittle full string comparisons unless the message is user-facing.

## Verification Focus

- Run `dotnet test` for the changed solution/project or the repository's configured test command.
- Run targeted integration/component tests when endpoints, EF queries, Blazor components, auth, or middleware changed.
- Confirm tests clean up database state, temp files, servers, DI scopes, async tasks, and disposable clients.
- Record skipped integration tests only when they require unavailable infrastructure and the task cannot reasonably provide it.

## Evidence Focus

- In the evidence summary, name the behavior verified and the .NET commands run.
