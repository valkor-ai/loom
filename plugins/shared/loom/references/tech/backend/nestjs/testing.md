# NestJS Testing Quality

This file applies NestJS TestingModule, unit, controller, service, and E2E test rules to task-owned changes.

## When To Use

- The task changes NestJS controllers, services, modules, guards, pipes, interceptors, DTOs, persistence adapters, or tests.
- Use this when Jest, TestingModule, mocked providers, Supertest E2E, app initialization, or module compilation checks are needed to prove behavior.
- If the task changes only pure TypeScript code outside NestJS wiring, use TypeScript testing references without this NestJS testing reference.

## Implementation Focus

- Use `Test.createTestingModule` for provider/controller tests and register only dependencies needed by the unit under test.
- Mock owned external boundaries such as repositories, HTTP clients, queues, email clients, and clocks. Do not mock the service method being tested.
- Use E2E tests with an initialized Nest application when route binding, global pipes, guards, interceptors, filters, or middleware behavior matters.
- Clear mocks and close the application after tests to avoid shared state and open handles.
- Use test database or repository fakes according to the repository's persistence strategy.
- Keep validation and auth behavior covered through HTTP when those concerns are part of the endpoint contract.

## Verification Focus

- Run `npm run lint`, `npm run test`, and E2E/test targets used by the repository when affected.
- Prove success, validation error, not found, conflict, auth denial, role denial, and DI/module compilation when relevant.
- Verify global ValidationPipe and guards in E2E tests when endpoint behavior depends on them.
- For service changes, assert repository/client calls and exception mapping.

## Evidence Focus

- In the evidence summary, name the proof type: service unit test, controller test, TestingModule compile, E2E Supertest, guard test, pipe validation test, mocked provider, or lint/test command.
