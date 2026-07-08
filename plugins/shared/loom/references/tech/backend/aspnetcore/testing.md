# ASP.NET Core Testing Quality

This file applies .NET build, unit test, integration test, and WebApplicationFactory rules to task-owned ASP.NET Core changes.

## When To Use

- The task changes ASP.NET Core endpoints, clean architecture handlers, EF Core data access, authentication, configuration, health checks, or tests.
- Use this when xUnit/NUnit/MSTest, WebApplicationFactory, TestServer, EF test databases, mocks, or integration probes are needed to prove behavior.
- If the task changes only pure C# code outside ASP.NET Core wiring, use C# testing references without this ASP.NET Core testing reference.

## Implementation Focus

- Choose the smallest proof that covers the risk: unit tests for domain/application handlers, persistence tests for EF mapping/query behavior, and WebApplicationFactory integration tests for HTTP/middleware/DI behavior.
- Use test data builders or clear fixtures. Avoid order-dependent tests and shared mutable database state.
- Prefer realistic test configuration for auth, EF provider, options binding, and middleware when those behaviors are the target.
- Use fakes/mocks at owned external boundaries; do not mock the class under test or EF behavior when provider mapping is the risk.
- Keep cancellation, async behavior, and error paths covered for I/O-heavy handlers.
- Run `dotnet build` and `dotnet test` for affected solution/project scope.

## Verification Focus

- Prove success, validation error, not found, conflict, auth denial, role/claim denial, and database side effects when relevant.
- Verify DI container startup when registering handlers, services, validators, options, health checks, or middleware.
- For EF changes, assert database state rather than only service return values.
- For Minimal API changes, verify status codes, headers, response DTO shape, and OpenAPI metadata when contract changes.

## Evidence Focus

- In the evidence summary, name the proof type: unit test, handler test, WebApplicationFactory integration test, EF provider test, auth middleware test, options binding test, or build/test command.
