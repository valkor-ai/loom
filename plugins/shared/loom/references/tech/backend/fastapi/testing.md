# FastAPI Async Testing Quality

Use this topic reference when `tech/backend/fastapi/testing.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md` and selected Python testing references. This file applies pytest, httpx, dependency override, and async database test rules to task-owned FastAPI changes.

## When To Use

- The task changes FastAPI endpoints, schemas, dependencies, async SQLAlchemy code, authentication, background tasks, WebSockets, settings, or tests.
- Use this when pytest-asyncio/anyio, httpx AsyncClient, ASGITransport, dependency overrides, async database fixtures, or OpenAPI checks are needed to prove behavior.
- If the task changes only pure Python code outside FastAPI wiring, use Python testing references without this FastAPI testing reference.

## Implementation Focus

- Match the app style: use `AsyncClient`/ASGITransport for async tests and TestClient only when the project intentionally uses sync tests.
- Override dependencies at the app boundary and clear overrides after each test. Do not mutate global app state across tests.
- Use isolated database fixtures with explicit setup/teardown, transaction rollback, or test schema recreation according to the repository's pattern.
- Build auth helper fixtures that generate real tokens when testing protected endpoints; mock auth only when auth itself is out of scope.
- Test schema validation through HTTP when endpoint behavior matters, and test service/CRUD functions directly when persistence behavior is the target.
- Avoid event-loop leaks, unawaited coroutines, and sync database calls in async tests.

## Verification Focus

- Run the repository's pytest target, including async endpoint and persistence tests touched by the task.
- Prove success, validation error, not found, conflict, auth denial, role denial, and database side effects when relevant.
- Verify OpenAPI route exposure when routes or response models change.
- For dependency changes, assert overrides are scoped and cleaned up.

## Evidence Notes

- Record `fastapi.testing` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/backend/fastapi/testing.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the proof type: async endpoint test, schema validation test, dependency override, async database fixture, auth fixture, OpenAPI check, or CRUD/service test.
