# FastAPI Async Testing

Use the smallest boundary that proves the task-owned behavior. This reference applies only when the task explicitly owns FastAPI test implementation; framework availability alone does not create testing work.

## When To Use

Use this reference for FastAPI endpoint, dependency, lifespan, exception-handler, security, WebSocket, background behavior, or async SQLAlchemy integration tests. Pure Python unit tests remain in the Python testing reference.

## Implementation Focus

### Test Boundary Selection

| Behavior | Preferred Boundary |
|---|---|
| Pure domain/service rule | Plain pytest with real small collaborators or focused fakes |
| HTTP routing/validation/error shape | `httpx.AsyncClient` with `ASGITransport` |
| Dependency composition | ASGI request with scoped dependency override |
| SQLAlchemy mapping/query/transaction | Async provider-backed persistence test |
| Authentication/authorization | Real security dependency and synthetic token/session |
| Lifespan resource | Client/lifespan manager that executes startup and shutdown |
| WebSocket | Framework test client or async client supporting the actual session lifecycle |

Do not start a database or full application for pure validation/calculation. Do not replace the behavior under test with a mock and then assert only the mock call.

### Async HTTP Client

Use one async test backend consistently. `ASGITransport` does not always execute lifespan automatically; use the repository's lifespan manager or application factory when startup resources matter.

```python
@pytest.fixture
async def client(app: FastAPI) -> AsyncIterator[AsyncClient]:
    async with LifespanManager(app):
        async with AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as http:
            yield http


@pytest.mark.anyio
async def test_conflicting_state_returns_contract_error(client: AsyncClient) -> None:
    response = await client.post("/orders/42/approve")
    assert response.status_code == 409
    assert response.json() == {
        "code": "ORDER_STATE_CONFLICT",
        "message": "Order cannot be approved from its current state",
    }
```

Use `pytest.mark.anyio` or the repository's established asyncio plugin, not a mixture of event-loop fixtures and markers. Treat unawaited-coroutine warnings and leaked tasks as failures.

### Dependency Overrides

Build the application per test or restore every override in `finally`/fixture teardown. The override must have a compatible dependency shape, including async/yield cleanup.

```python
@pytest.fixture
def override_actor(app: FastAPI, actor: Actor) -> Iterator[None]:
    app.dependency_overrides[require_actor] = lambda: actor
    try:
        yield
    finally:
        app.dependency_overrides.pop(require_actor, None)
```

Do not call `dependency_overrides.clear()` in a shared suite when other fixtures may own overrides. Avoid mutable global application state across parallel tests.

### Persistence Fixtures

Use the selected provider or a repository-approved compatible test provider for dialect-sensitive behavior. Keep engine/session lifecycle scoped and deterministic. Transaction rollback fixtures are useful only when the behavior does not depend on commit, post-commit events, constraint timing, or another connection observing data.

Run migrations when migration/runtime compatibility is under test. `metadata.create_all()` proves mappings can create tables, not that migrations are complete.

### Security Fixtures

Use synthetic credentials with real validation for security behavior. Cover missing, invalid, expired, wrong-type, denied, and allowed cases. Override current-actor dependencies only when the test explicitly targets downstream business behavior rather than auth wiring.

### Background Work And WebSockets

For `BackgroundTasks`, assert the follow-up effect or failure signal after the response lifecycle. For durable jobs, test the producer boundary rather than pretending in-process background work is durable.

WebSocket tests should cover authentication, accept/reject, message validation, disconnect cleanup, and bounded broadcast behavior owned by the task.

### Assertions And Isolation

Assert complete contract outcomes: status, body, headers, persisted state, emitted command, or stable error. Avoid tests that only assert non-null, list length, no exception, or mock invocation.

Freeze or inject time, IDs, and randomness. Do not use arbitrary sleeps; use completion signals, events, or virtual time where supported.

## Verification Focus

- Run the narrow test module or marker for changed FastAPI behavior, then the owning package suite when risk crosses shared dependencies.
- Cover success and important validation, not-found, conflict, auth, and dependency-failure paths.
- Verify dependency overrides and lifespan resources are restored after each test.
- Prove database commit/readback when durability or another request observes the result.
- Inspect changed OpenAPI operations when router or schema exposure changes.

## Evidence Focus

Record the exact test boundary, scenario, command, and meaningful assertion. Test count or a passing application startup does not prove the task's route, failure, persistence, security, or cleanup behavior.

## Unsafe Defaults

- Sync `TestClient` mixed into an intentionally async suite without a reason.
- `ASGITransport` assumed to run lifespan automatically.
- Global dependency overrides leaked between tests.
- SQLite used as proof for provider-specific SQL or locking.
- Security filters/dependencies bypassed in the only endpoint test.
- Test-level rollback used to prove post-commit behavior.
- Arbitrary sleeps, shared mutable fixtures, or order-dependent tests.
