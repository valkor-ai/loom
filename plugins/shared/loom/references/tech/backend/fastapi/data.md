# FastAPI Async Data Quality

This file applies async SQLAlchemy, session, migration, and CRUD rules to task-owned persistence behavior.

## When To Use

- The task changes SQLAlchemy models, async sessions, database dependencies, CRUD/query functions, transactions, Alembic migrations, relationship loading, or database-backed business rules.
- Use this when async database I/O, transaction ownership, query shape, or migration behavior affects correctness.
- If the task only changes FastAPI routing or schemas with no persistence behavior, do not load this data reference.

## Implementation Focus

- Use async SQLAlchemy APIs consistently with an async driver. Do not call synchronous engine/session operations inside async endpoints.
- Centralize session creation and dependency injection. Make commit/rollback ownership explicit so endpoint, service, and CRUD layers do not fight over transaction boundaries.
- Use typed mapped columns, constraints, indexes, relationships, and loading strategies that match the domain and API read patterns.
- Prefer `select`, `scalars`, `scalar_one_or_none`, `selectinload`, and explicit pagination filters over ad hoc ORM access.
- Use `flush`/`refresh` deliberately when generated IDs or defaults are needed before returning.
- Keep Alembic migrations aligned with model changes. Do not rely on `create_all` for production schema evolution.
- Handle uniqueness, not-found, conflict, and integrity errors at the service/API boundary with stable error mapping.
- Keep database URLs and pool/runtime settings in configuration, not hardcoded module constants.

## Verification Focus

- Run async persistence tests with the repository's pytest/httpx/database fixture strategy.
- Prove write/read round-trip, generated IDs, defaults, constraints, relationship loading, pagination/filtering, and rollback behavior when touched.
- For migrations, verify a clean database can upgrade and the app starts against the configured test database.
- Test async behavior without leaking sessions or leaving dependency overrides active across tests.

## Evidence Focus

- In the evidence summary, name the async data decision: session dependency, transaction boundary, SQLAlchemy model, query shape, relationship loading, migration, integrity handling, or persistence proof.
