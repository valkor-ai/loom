# FastAPI Routing And Application Boundaries

Implement the accepted HTTP interfaces without redesigning methods, paths, payloads, status codes, or error semantics. This reference owns FastAPI router composition, dependency boundaries, endpoint lifecycle, and transport behavior.

## When To Use

Use this reference when a task owns FastAPI routes, router composition, HTTP dependencies, endpoint error translation, background responses, WebSockets, or OpenAPI exposure. Pure Python services and persistence-only tasks do not need it.

## Implementation Focus

### Application And Router Ownership

Create the `FastAPI` application in one composition root. Register routers, exception handlers, middleware, and lifespan resources there or through repository-standard application factories. Avoid importing a global application object into domain or persistence modules.

Group routes by business capability with `APIRouter`. Prefixes and tags follow the accepted interface and repository layout; do not invent `/v1` or change trailing-slash behavior because an example uses it.

```python
router = APIRouter(prefix="/orders", tags=["orders"])

DbSession = Annotated[AsyncSession, Depends(get_db_session)]
CurrentActor = Annotated[Actor, Depends(require_actor)]

@router.post("", response_model=OrderRead, status_code=status.HTTP_201_CREATED)
async def create_order(
    command: OrderCreate,
    session: DbSession,
    actor: CurrentActor,
) -> OrderRead:
    created = await order_service.create(session, actor, command)
    return OrderRead.model_validate(created)
```

### Endpoint And Service Boundary

Endpoint functions own transport work: parameter binding, dependency resolution, request validation, one application operation, and response mapping. Business state transitions, ownership rules, transaction decisions, provider retries, and multi-record invariants belong in services or domain components.

Use `Annotated` aliases for stable shared dependencies. Do not hide materially different authorization or transaction behavior behind one vague dependency alias. Dependency functions must declare cleanup with `yield` when they own request-scoped resources.

### Async And Blocking Work

`async def` is useful only when the call chain performs awaitable I/O. Never invoke synchronous database clients, HTTP clients, filesystem calls, or CPU-heavy work directly on the event loop. Keep a synchronous endpoint for bounded synchronous work, use the repository's thread-pool boundary deliberately, or select an async adapter end to end.

Do not call `asyncio.create_task` for required business work. FastAPI `BackgroundTasks` execute after the response in the same process and are not durable. Use them only for bounded, loss-tolerant follow-up with observable failure; durable work requires an accepted job or messaging boundary.

### Parameters And Collections

Use typed `Path`, `Query`, `Header`, and body models for transport constraints. Keep path parameter declarations ahead of conflicting dynamic routes. Validate filter and sort fields against explicit allowlists.

Unbounded collections require the accepted pagination contract, deterministic sorting, bounded maximum size, and stable metadata. Empty collections return the declared successful collection shape rather than a not-found error.

### Error Translation

Translate domain/application failures through focused exception handlers or a consistent endpoint boundary. Preserve the accepted error body and distinguish validation, not found, conflict, unauthenticated, forbidden, dependency unavailable, and unexpected failures.

```python
@app.exception_handler(OrderConflict)
async def order_conflict_handler(
    request: Request,
    error: OrderConflict,
) -> JSONResponse:
    return JSONResponse(
        status_code=status.HTTP_409_CONFLICT,
        content={"code": error.code, "message": error.user_message},
    )
```

Never return raw SQLAlchemy, JWT, provider, traceback, file-path, or class-name details. Do not catch every exception in every endpoint; unexpected failures should reach the one boundary selected by `tech/code/observability.md` when this task owns observability.

### Lifespan, Streaming, And WebSockets

Use FastAPI lifespan for shared clients and resources with explicit startup failure and shutdown behavior. Do not create schemas or seed business data during application startup when migrations own schema evolution.

Streaming responses need bounded producers, cancellation cleanup, media type, and disconnect behavior. WebSockets need authentication before accepting protected sessions, connection ownership, message size limits, backpressure, disconnect cleanup, and a defined multi-instance delivery model.

### OpenAPI Contract

Set `response_model`, status code, parameter constraints, summaries, and documented errors when they are part of the accepted contract. Exclude internal/admin routes deliberately. Generated OpenAPI is evidence of transport shape, not proof that business behavior works.

## Verification Focus

- Exercise accepted success and blocking paths through `httpx`/ASGI transport.
- Assert exact status, response/error shape, headers, pagination, filtering, and sorting owned by the task.
- Verify router inclusion, path precedence, dependency cleanup, and exception-handler registration.
- Test lifespan or WebSocket behavior through a client that actually runs the relevant lifecycle.
- Inspect OpenAPI operations and schemas for changed public routes.

## Evidence Focus

Identify the route, dependency, exception boundary, or lifecycle resource changed and the test that proves its externally visible behavior. A route appearing in OpenAPI or returning any `2xx` response is not sufficient evidence for validation, authorization, transaction, or failure semantics.

## Unsafe Defaults

- Repository calls and state transitions directly in endpoint functions.
- Blocking I/O inside `async def`.
- Global mutable dependency state shared across requests.
- Unbounded list endpoints or arbitrary client-selected sort fields.
- Durable business work scheduled with `BackgroundTasks` or `create_task`.
- Catch-all exception handlers that expose internal messages or convert failures to success.
- Adding versioned prefixes that are absent from the accepted interface.
