# FastAPI Async SQLAlchemy Data Access

This reference applies only when the accepted stack uses SQLAlchemy. It owns async engine/session integration, transaction boundaries, query behavior, and migration alignment for FastAPI applications; provider SQL remains in the selected SQL references.

## When To Use

Use this reference for SQLAlchemy 2.x models, async sessions, repository/query components, transaction behavior, relationship loading, Alembic integration, and provider-backed persistence tests. Do not apply it to Tortoise ORM, Django ORM, Spring Data, jOOQ, or persistence-free API work.

## Implementation Focus

### Engine And Session Lifetime

Create one async engine and `async_sessionmaker` for the application lifecycle. The URL must use the selected async driver. Configure pool behavior from runtime requirements rather than copying production-sized defaults into every project.

```python
engine = create_async_engine(settings.database_url, pool_pre_ping=True)
session_factory = async_sessionmaker(engine, expire_on_commit=False)

async def get_db_session() -> AsyncIterator[AsyncSession]:
    async with session_factory() as session:
        yield session
```

The request dependency owns opening and closing the session, not an automatic commit of every request. Application/service operations should make the transaction boundary explicit with `async with session.begin()` or a repository-standard unit-of-work boundary. Do not let endpoint, service, and dependency layers each commit independently.

Never share an `AsyncSession` across concurrent tasks or requests. Do not place it in module-global mutable state. Dispose the engine during application lifespan shutdown.

### Mapping Boundary

Use SQLAlchemy 2.x typed mappings with `Mapped` and `mapped_column`. Align identifiers, nullability, lengths, enums, decimals, timestamps, defaults, foreign keys, unique constraints, indexes, and version fields with accepted migrations and provider behavior.

Keep ORM entities out of public response contracts. Pydantic response mapping happens while required data is loaded and the session boundary is understood. Avoid implicit lazy loads from serialization.

### Query Shape And Loading

Use `select()` and explicit scalar/cardinality operations:

- `scalar_one()` when exactly one row is required
- `scalar_one_or_none()` for unique optional lookup
- `scalars().all()` for an intentionally bounded result
- `selectinload` for bounded relationship collections
- `joinedload` for suitable single-valued relationships

Define pagination, deterministic ordering, filter allowlists, and projection shape at the query boundary. Do not load an unbounded table and slice it in Python. Avoid N+1 access by matching loader strategy to the actual list/detail read path.

### Write And Transaction Semantics

Use `flush()` when generated identifiers or constraint timing are needed before commit; use `refresh()` only for values that must be reloaded. A flush is not a durable commit.

Bulk `update()`/`delete()` bypass normal object state and lifecycle handling. Specify synchronization/version behavior and do not return stale in-memory objects after a bulk operation.

Map known uniqueness, foreign-key, optimistic-concurrency, and integrity failures to stable application outcomes after rollback. An existence check can improve feedback but does not replace the database constraint under concurrency.

External HTTP, email, payment, or messaging side effects are not atomic with a SQL transaction. Use an accepted post-commit, outbox, idempotency, or compensation boundary.

### Async Correctness

Do not call synchronous engines, sessions, drivers, or blocking provider helpers from async request paths. Avoid hidden I/O through attribute access. Await every database operation and keep independent concurrent queries on independent sessions.

Cancellation and timeout can interrupt application waiting while the database operation has uncertain state. Define transaction rollback and idempotency for retry-sensitive writes.

### Alembic And Startup

Alembic or the selected migration tool owns production schema evolution. Do not call `metadata.create_all()` during normal application startup as a replacement for migrations.

Keep migration configuration aligned with the runtime model metadata and provider URL without importing the entire FastAPI application. Separate schema changes, backfills, constraint activation, and cleanup when one step cannot be applied safely.

## Verification Focus

- Run async write/flush/commit/readback tests against the selected or compatible repository test provider.
- Prove IDs, defaults, enums, decimals, timestamps, relationships, constraints, and version behavior touched by the task.
- Test query cardinality, filters, ordering, pagination, eager loading, and empty results.
- Verify rollback and session cleanup after expected and unexpected failures.
- Upgrade a clean schema through migrations and start mappings against it when migrations change.
- Use provider-specific tests for native SQL, generated values, locking, JSON/array types, or dialect behavior.

## Evidence Focus

Identify the session/transaction owner, query shape, migration, or integrity behavior and the provider-backed assertion that proves it. In-memory object assertions before commit do not prove schema, constraint, transaction, or readback behavior.

## Unsafe Defaults

- Automatic commit hidden in a request dependency for multi-step workflows.
- One `AsyncSession` shared across concurrent tasks.
- Synchronous SQLAlchemy calls in `async def` endpoints.
- ORM entities returned directly through the API.
- Lazy relationship access during Pydantic serialization.
- Unbounded `scalars().all()` list queries.
- `create_all()` used as production migration behavior.
- SQLite-only tests claimed as proof for provider-specific SQL.
