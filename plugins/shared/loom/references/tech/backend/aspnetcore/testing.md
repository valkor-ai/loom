# ASP.NET Core Testing

Use the repository's selected test framework and the narrowest boundary that proves the task. ASP.NET Core work does not automatically require every test type; framework testing guidance is selected only for tasks that own test implementation.

## Proof Boundary

| Claim | Suitable proof |
|---|---|
| Domain invariant/value object | Plain unit test |
| Application handler/orchestration | Unit test with owned ports mocked/faked |
| EF mapping/query/transaction | Selected-provider integration test |
| DI/options registration | Focused service-provider/host startup test |
| Route, middleware, filters, auth, serialization | `WebApplicationFactory<Program>` HTTP test |
| Published runtime/AOT behavior | Published artifact smoke/integration test |

Calling a Minimal API delegate or controller method directly cannot prove route binding, global validation, middleware, exception handlers, authorization, response serialization, or host configuration.

## Unit And Handler Tests

Construct domain/application components directly when framework DI is irrelevant. Mock repositories, clocks, queues, HTTP ports, identity, and other owned boundaries; do not mock the rule/handler being tested.

Assert returned values plus state transitions, calls, absence of forbidden side effects, cancellation, and typed failures. Keep time, IDs, and randomness deterministic through explicit ports where outcomes depend on them.

Use test data builders/factories for readable valid defaults and override only scenario-relevant fields. Avoid one giant shared fixture whose mutation creates order dependence.

## WebApplicationFactory Fidelity

Derive a focused factory from the actual entry point and preserve production service registration, middleware, route groups, JSON options, validation, auth, and exception handling. Override only external/runtime dependencies outside the test's claim.

```csharp
public sealed class ApiFactory : WebApplicationFactory<Program>
{
    protected override void ConfigureWebHost(IWebHostBuilder builder) =>
        builder.ConfigureTestServices(services =>
        {
            services.RemoveAll<IClock>();
            services.AddSingleton<IClock>(new FrozenClock(TestTime.UtcNow));
        });
}
```

Use the factory client to assert exact status, headers, problem details, JSON shape, and durable effects. Dispose clients/factories and avoid shared mutable host/database state between tests.

Do not replace authentication in the only security test. A stable test scheme may be used for non-auth behavior, while dedicated auth tests execute the selected real scheme/policy and denied paths.

## EF Core Tests

Use the accepted provider when testing constraints, SQL translation, decimals, JSON, collations, migrations, transactions, locking, or concurrency. EF InMemory does not behave like a relational database, and SQLite is not a universal substitute for SQL Server/PostgreSQL/MySQL.

Isolate database state per test through transactions, schemas/databases, or deterministic cleanup compatible with the provider. Assert database readback and rollback, not only in-memory tracked entities.

Use containers/shared dependencies only for suites that own provider fidelity, and reuse infrastructure according to the repository harness without leaking data across cases.

## Configuration, Health, And Hosted Services

Host startup tests should cover valid options and missing/invalid mandatory settings. Health tests should prove liveness/readiness classification and dependency transitions.

For `BackgroundService`, create deterministic completion signals and cancellation. Do not wait arbitrary wall-clock delays. Verify scope creation, bounded retries, idempotency, shutdown, and resource cleanup where owned.

Check for open servers, database connections, timers, consumers, and unobserved tasks after tests. Open-handle/resource warnings are failures to understand, not noise to suppress globally.

## HTTP Contract Coverage

For changed operations, cover the relevant set of success, malformed input, not found, conflict/concurrency, unauthenticated, forbidden/wrong owner, dependency unavailable, and cancellation behavior.

Assert pagination bounds/order, conditional headers, location, content type, and sensitive-field exclusion when declared. Test list isolation separately from detail/mutation authorization.

OpenAPI snapshots can detect contract drift when the repository uses them, but they do not replace behavior tests.

## Verification Commands

Run the changed test project/filter first, such as `dotnet test tests/Orders.Tests --filter FullyQualifiedName~ApproveOrder`. Then run the owning project/solution build or focused suite when shared contracts, DI, middleware, or project references changed. Preserve repository configuration and target framework flags.

## Delivery Evidence

Record the test boundary, scenario, command, and meaningful assertion. A passing `dotnet test` count alone does not prove real middleware, authorization, provider semantics, startup validation, migration safety, or resource cleanup.

## Unsafe Defaults

- Integration tests added for every pure rule.
- Direct endpoint calls claimed as HTTP pipeline evidence.
- Real auth replaced in the only authorization test.
- EF InMemory/SQLite claimed as proof of selected-provider semantics.
- Shared mutable factory/database state and order-dependent tests.
- Arbitrary sleeps for hosted/background behavior.
- OpenAPI snapshots used as the only route behavior proof.
