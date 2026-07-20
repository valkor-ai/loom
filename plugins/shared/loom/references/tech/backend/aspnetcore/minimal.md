# ASP.NET Core Minimal APIs

Implement accepted HTTP interfaces through Minimal APIs only when the selected stack and repository use that endpoint style. Preserve methods, paths, schemas, statuses, errors, auth policy, and exposure rules rather than redesigning the API around framework examples.

## Endpoint Organization

Group cohesive endpoints through `MapGroup` or a focused `Map...Endpoints` extension. The effective path combines the application's base path, group prefix, and endpoint pattern; avoid duplicate or missing `/api` segments.

```csharp
public static IEndpointRouteBuilder MapOrderEndpoints(this IEndpointRouteBuilder routes)
{
    var orders = routes.MapGroup("/api/orders").WithTags("Orders");

    orders.MapPost("/", CreateOrder)
        .WithName("CreateOrder")
        .Produces<OrderResponse>(StatusCodes.Status201Created)
        .ProducesValidationProblem()
        .ProducesProblem(StatusCodes.Status409Conflict);

    orders.MapGet("/{id:guid}", GetOrder)
        .Produces<OrderResponse>()
        .ProducesProblem(StatusCodes.Status404NotFound);
    return routes;
}
```

Do not place all endpoints in `Program.cs` once route ownership becomes difficult to inspect. Do not mix controllers and Minimal APIs within one capability unless the repository has a deliberate boundary.

## Binding And DTOs

Use explicit route constraints and typed path/query/header/body parameters. `[AsParameters]` can group query values, but the resulting type must keep defaults, nullability, validation, and OpenAPI shape aligned with the contract.

Use request and response records/classes rather than EF/domain entities. Keep server-owned actor, tenant, state, audit, and generated fields out of client-writable models.

Use the established validation strategy: endpoint filters, FluentValidation integration, data annotations, or application validation. Transport validation does not replace uniqueness, ownership, state-transition, or concurrent database constraints.

## Typed Results And Status Semantics

Prefer typed results when they improve compile-time response metadata:

```csharp
static async Task<Results<Ok<OrderResponse>, NotFound, ProblemHttpResult>> GetOrder(
    Guid id,
    IOrderQueries queries,
    CancellationToken ct)
{
    var order = await queries.Find(id, ct);
    return order is null ? TypedResults.NotFound() : TypedResults.Ok(order);
}
```

Return `Created` with the accepted location when a retrievable resource is created. Keep `204` bodyless. Preserve declared headers such as ETag, Location, Retry-After, pagination links, or idempotency outcomes.

Translate expected application failures once through `IExceptionHandler`, Problem Details, or the established endpoint boundary. Avoid catch-all filters per endpoint and never expose stack traces, SQL messages, token errors, or internal type names.

## Dependency Injection And Cancellation

Bind application services/handlers and trusted current-user abstractions through DI. Endpoint delegates should perform transport mapping and invoke one application operation, not query `DbContext`, coordinate external calls, or own transactions.

Accept `CancellationToken` and propagate it through EF Core, HTTP clients, streams, and application handlers. Do not convert cancellation into a generic `500` or swallow it while continuing side effects.

## Route Metadata And OpenAPI

Use names, tags, summaries, `Produces`, auth requirements, and OpenAPI metadata when the repository publishes a contract. Metadata must reflect runtime behavior; `.Produces(404)` does not make the endpoint return not found.

Do not add Swagger UI, API versioning, or development server URLs from convention. Those are bootstrap/API-contract decisions and should remain environment-aware.

## Filters, Middleware, And Policies

Endpoint filters are appropriate for endpoint-local validation or reusable transport behavior. Authentication/authorization should use ASP.NET policies and endpoint/group metadata. Global exception handling, CORS, rate limiting, output caching, and request logging belong in the application pipeline.

Keep middleware order deliberate and avoid reimplementing middleware behavior inside filters. Route-group authorization can be narrowed or made public only through explicit accepted policy.

## Collections And Conditional Requests

Bound page size and allowlist filtering/sorting fields. Use deterministic ordering and stable response metadata. Empty collections normally return the successful collection shape.

Implement ETag/If-Match/cache semantics only when declared. Output caching is not safe by default for authenticated, tenant-specific, mutable, or user-varying responses.

## Verification

- Use `WebApplicationFactory<Program>` or the repository's real test host to exercise route registration and global pipeline behavior.
- Assert exact success/error status, body, headers, binding, validation, and response-field exclusion.
- Test not-found, conflict, authentication, authorization, cancellation, pagination, and conditional behavior owned by the interface.
- Verify route names/OpenAPI only when the published contract changes.
- Build the owning project so delegate signatures and typed result unions compile.

## Delivery Evidence

Name the effective route and test-host request proving binding, typed result, headers, and relevant policies. Calling the static endpoint method directly cannot prove route groups, middleware, global handlers, authentication, validation filters, or OpenAPI metadata.

## Unsafe Defaults

- Minimal API guidance loaded for a controller-based stack.
- EF Core queries and business transitions inside endpoint delegates.
- Domain/EF entities returned directly.
- Status metadata treated as runtime behavior.
- Swagger, versioning, or `/api` prefixes added without an accepted contract.
- `CancellationToken` ignored across I/O calls.
- Output caching enabled for personalized or mutation responses.
