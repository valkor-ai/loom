# ASP.NET Core Minimal API Quality

This file applies ASP.NET Core Minimal API and endpoint routing rules to task-owned HTTP behavior.

## When To Use

- The task changes Minimal API endpoints, route groups, endpoint filters, request/response DTOs, middleware ordering, OpenAPI metadata, or status/error mapping.
- Use this when HTTP route shape, validation, DI binding, or endpoint response behavior affects correctness.
- If the task only changes EF Core, domain logic, or clean architecture handlers with no HTTP surface, do not load this minimal API reference.

## Implementation Focus

- Keep endpoints thin: bind route/query/body inputs, call an application service or handler, map outcomes to typed HTTP results, and return DTOs.
- Use route groups for shared prefix, tags, authorization, filters, and OpenAPI metadata. Do not scatter equivalent middleware and metadata across unrelated endpoints.
- Use record DTOs for request/response models and keep EF/domain entities out of API responses.
- Validate mutating requests with the repository's established strategy, such as FluentValidation endpoint filters or explicit validation results.
- Use async endpoint delegates with `CancellationToken` for I/O work. Do not call synchronous database or HTTP APIs from endpoints.
- Make status codes explicit with `Results`, `TypedResults`, `Produces`, `ProducesProblem`, and created-location behavior when creating resources.
- Keep OpenAPI names, tags, response metadata, and route constraints aligned with the accepted API contract.

## Verification Focus

- Run endpoint or integration tests with the repository's ASP.NET Core test host strategy.
- Prove success, validation problem, not found, conflict, auth denial, pagination/filtering, and response DTO shape when touched.
- Verify route group metadata and OpenAPI generation when public endpoint contract changes.
- Run `dotnet build` and the relevant `dotnet test` target after endpoint changes.

## Evidence Focus

- In the evidence summary, name the ASP.NET Core API decision: endpoint boundary, route group, DTO mapping, validation filter, typed result, OpenAPI metadata, middleware order, or endpoint proof.
