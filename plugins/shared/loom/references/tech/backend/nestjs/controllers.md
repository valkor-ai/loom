# NestJS Controllers And HTTP Routing

Controllers adapt the accepted HTTP contract to application services. They own route binding, transport validation entry points, status and header behavior, and response mapping; they do not own persistence or business workflows.

## Controller Boundary

Group endpoints by cohesive resource or capability and preserve the repository's module structure. Inject providers through the constructor and keep handlers short enough that transport behavior is visible without hiding domain decisions in decorators.

Bind every accepted input explicitly:

- `@Param` for path identifiers with the matching parse pipe
- a query DTO for filtering, ordering, cursor/page controls, and optional query values
- `@Body` with a dedicated command/input DTO
- `@Headers` only for contract-owned headers such as idempotency or preconditions
- trusted identity from the established guard/decorator boundary, never from client-owned body fields

Do not pass raw `Request`, `req.body`, or arbitrary query objects into services when Nest decorators and DTOs can express the contract. Use platform request/response objects only for transport behavior that Nest abstractions cannot represent cleanly, such as streaming or a provider-specific callback.

## Route Composition

Compose the effective path from the configured global prefix, controller path, and method path. Match the accepted interface exactly, including parameter names, trailing-slash convention, and nested-resource ownership.

Use `ParseUUIDPipe`, `ParseIntPipe`, enum pipes, or focused custom pipes for primitive route values. A parse failure should produce the repository's validation envelope rather than reaching the service with an invalid identifier.

Nested routes should establish parent ownership in the service/query boundary. Receiving both `projectId` and `itemId` does not prove the item belongs to that project.

Do not add a global prefix or URI/header versioning from framework convention. Configure either only when the accepted API contract and existing bootstrap own it.

```typescript
@Controller("orders")
export class OrdersController {
  constructor(private readonly orders: OrdersService) {}

  @Post()
  async create(
    @Body() input: CreateOrderDto,
    @CurrentActor() actor: Actor,
  ): Promise<OrderResponseDto> {
    const created = await this.orders.create(actor, input);
    return OrderResponseDto.from(created);
  }

  @Get(":id")
  findOne(@Param("id", ParseUUIDPipe) id: string) {
    return this.orders.findOne(id);
  }
}
```

The custom identity decorator above represents an already-selected authentication boundary; do not create it for an unauthenticated interface.

## Status, Headers, And Responses

Use Nest defaults only when they match the contract. Apply `@HttpCode` for non-default success semantics and keep `204` responses bodyless. Set location, pagination, cache, retry, idempotency, ETag, or precondition headers only when declared or already established.

Return explicit response DTOs or mapped transport objects. Do not return ORM entities when they expose persistence fields, lazy relations, credentials, internal state, or provider-specific values.

Keep expected failure mapping stable across controllers. Use typed application/domain failures plus the existing exception filter rather than broad `try/catch` blocks in every handler. Preserve distinctions among validation, not found, conflict, authentication, authorization, rate limiting, and unavailable dependencies.

## Pipes, Guards, Interceptors, And Filters

Choose the Nest extension point by responsibility:

| Concern | Boundary |
|---|---|
| Parse and transport validation | Pipe |
| Authentication or operation authorization | Guard |
| Cross-cutting request/response behavior | Interceptor |
| Exception-to-response translation | Exception filter |
| Reusable application behavior | Provider/service |

Apply global behavior in bootstrap/module providers only when it is truly application-wide. Method/controller overrides must remain visible and must not silently bypass global security, validation, or serialization behavior.

Interceptors may shape successful responses or add telemetry, but they should not obscure endpoint-specific status/body contracts. Exception filters must not leak stack traces, database errors, tokens, or internal provider messages.

## OpenAPI Alignment

When the repository publishes OpenAPI, keep operation, parameter, request, response, and error metadata aligned with the accepted interface. Prefer DTO-driven schemas and explicit response decorators for status variants. Do not invent documentation-only fields or make runtime validation diverge from the generated schema.

Swagger bootstrap, server URLs, auth schemes, and document exposure are application-level decisions. Do not add them to a feature controller unless the task owns that setup.

## Verification

- Compile the owning module so route metadata and dependency injection are resolved.
- Exercise the real Nest application for route composition, global prefix/pipes/guards/filters, and adapter behavior.
- Assert success status/body/headers plus owned validation, not-found, conflict, and authorization branches.
- Verify path and query parsing, nested-resource ownership, pagination bounds, and response field exclusion.
- Generate or inspect OpenAPI only when the task changes a published interface.

## Delivery Evidence

Name the controller and effective route, then identify the HTTP assertions proving binding, status, response mapping, and relevant cross-cutting behavior. A controller unit call cannot prove global pipe, guard, interceptor, filter, prefix, or adapter behavior.

## Unsafe Defaults

- Business rules or repository calls spread through controller handlers.
- Raw request payloads forwarded into providers.
- `@Res()` used for ordinary JSON responses, disabling Nest response handling.
- Global prefix/versioning introduced without an accepted contract.
- ORM entities returned as public response models.
- OpenAPI decorators treated as runtime validation.
