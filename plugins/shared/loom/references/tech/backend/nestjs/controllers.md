# NestJS Controller Quality

This file applies NestJS controller, route, pipe, and Swagger rules to task-owned HTTP behavior.

## When To Use

- The task changes NestJS controllers, routes, route parameters, query handling, guards at controller/method level, response status codes, Swagger decorators, or global prefix/version behavior.
- Use this when route shape, request binding, response mapping, or API documentation affects correctness.
- If the task only changes services/providers with no HTTP surface, do not load this controller reference.

## Implementation Focus

- Keep controllers thin: bind `@Param`, `@Query`, and `@Body`, call a service, map response/status behavior, and leave business logic to providers.
- Use DTOs for request bodies and query objects. Do not pass raw request bodies into services.
- Use parse pipes for primitive route/query parameters and ValidationPipe-backed DTOs for structured input.
- Keep Swagger decorators aligned with the accepted API contract: tags, operation summary, response types, error responses, params, and query options.
- Use explicit `@HttpCode` when default status codes do not match the contract.
- Apply guards at the narrowest appropriate scope and keep public/private route intent visible.
- Avoid adding URI versioning or broad global prefixes unless the accepted API contract owns them.

## Verification Focus

- Run controller unit tests or E2E tests with Supertest according to repository convention.
- Prove success, validation error, not found, auth denial, forbidden, pagination/query behavior, and response DTO shape when touched.
- Verify Swagger/OpenAPI generation when public route contract changes.
- Run lint/build/test targets for affected NestJS project scope.

## Evidence Focus

- In the evidence summary, name the NestJS controller decision: controller boundary, route binding, pipe usage, guard scope, status code, Swagger metadata, or E2E route proof.
