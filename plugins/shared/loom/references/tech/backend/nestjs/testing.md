# NestJS Testing Patterns

Choose the narrowest test boundary that proves the changed behavior. Use the repository's established runner and Nest bootstrap helpers; do not replace Jest with another runner or add E2E infrastructure solely because an example uses it.

## Test Boundary Selection

| Behavior to prove | Preferred boundary |
|---|---|
| Pure rule or mapper | Plain TypeScript unit test |
| Service orchestration | `TestingModule` with owned ports mocked/faked |
| Provider token/module wiring | Focused module compilation test |
| Pipe, guard, interceptor, filter | Direct unit test plus HTTP test when registration matters |
| Route/global bootstrap behavior | Nest application HTTP integration/E2E test |
| ORM constraint/transaction/query | Selected-provider integration test |

Controller method calls prove delegation and mapping only. They do not execute route metadata, global pipes, guards, interceptors, exception filters, middleware, serializers, or the HTTP adapter.

## TestingModule Construction

Register the unit under test and its actual injection tokens. Mock only owned boundaries such as repositories, HTTP clients, queues, clocks, mail, object storage, and event publishers. Do not mock the service method or policy being tested.

Use typed mocks or protocol fakes so signature drift fails compilation. Reset call history and implementation state between cases; a module-scoped mutable mock can leak results and order dependencies.

Override providers/guards only when the overridden concern is outside the test's claim. An auth test must execute the real selected guard/strategy; a controller contract test may supply a stable authenticated principal through an established test harness.

Compile representative modules when imports, exports, dynamic-module options, provider tokens, scopes, or `forwardRef` behavior changes. A minimal unit module can accidentally hide missing production imports.

## HTTP Application Fidelity

Create the test application through the same bootstrap/configuration function used by production when possible. Otherwise mirror all contract-relevant global configuration explicitly:

- global prefix and adapter
- `ValidationPipe` options
- guards and public metadata
- interceptors and serialization
- exception filters and error envelope
- CORS, cookies, shutdown hooks, and versioning only when owned

Initialize before Supertest requests and always close the application. Avoid binding a network port unless a dependency requires it; `app.getHttpServer()` is sufficient for ordinary HTTP E2E tests.

```typescript
const moduleRef = await Test.createTestingModule({ imports: [AppModule] }).compile();
const app = moduleRef.createNestApplication();
configureApplication(app); // same global pipes, filters, guards, and prefix
await app.init();

await request(app.getHttpServer())
  .post("/api/orders")
  .send(validInput)
  .expect(201);

await app.close();
```

## Behavioral Coverage

Assert exact accepted status, body, headers, and durable side effects. Cover the relevant failure classes: malformed input, not found, conflict/concurrency, unauthenticated, forbidden/wrong owner, dependency failure, and rollback.

For pagination/filtering, assert deterministic ordering, bounds, empty results, and metadata. For sensitive responses, assert forbidden fields are absent on the real serialization path.

State-transition tests should prove both the resulting state and rejected transitions. External-effect tests should prove call ordering/idempotency only when that is part of the operation contract.

## Persistence And Transactions

Repository mocks do not prove ORM mapping, database constraints, transaction rollback, locking, migrations, or provider-specific values. Use the accepted provider for those claims and isolate data per test through the repository's fixture/transaction strategy.

Do not silently substitute SQLite for PostgreSQL/MySQL behavior involving JSON, decimals, collations, indexes, locking, or generated values. Keep test containers or shared services bounded to suites that own such evidence.

## Time, Async Work, And Cleanup

Use fake timers only around code designed for them and restore real timers. Prefer injected clocks and deterministic IDs/randomness for business outcomes.

Await asynchronous assertions and background completion signals. Do not use arbitrary sleeps to make queue/event tests pass. Close database clients, queues, consumers, servers, and applications so open-handle warnings remain actionable.

For Observable-based providers/interceptors, assert completion/error behavior and unsubscribe where needed; do not leave hanging streams.

## Verification Commands

Run the changed test file or project target first. Then run the owning package's focused typecheck/lint/test target when shared module metadata, bootstrap, or provider contracts changed. Use the repository's E2E command/config rather than assuming `npm run test:e2e` exists.

## Delivery Evidence

Record the test boundary, scenario, command, and meaningful assertion. A passing suite count does not by itself prove global Nest configuration, denied authorization, durable persistence, transaction behavior, or resource cleanup.

## Unsafe Defaults

- E2E tests for every pure function or unit branch.
- Controller unit calls claimed as HTTP contract evidence.
- Real guard replaced in the only authorization test.
- Shared mutable mocks and test-order dependence.
- Test bootstrap diverging from production global configuration.
- ORM mocks claimed as transaction or constraint evidence.
- Arbitrary sleeps, unawaited promises, or unclosed applications.
