# FastAPI Routing Quality

Use this topic reference when `tech/backend/fastapi/routing.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`, selected schema references, and selected API references. This file applies APIRouter, dependency injection, response, and endpoint rules to task-owned HTTP behavior.

## When To Use

- The task changes FastAPI routers, endpoints, dependencies, path/query parameters, response models, status codes, OpenAPI tags, background tasks, WebSockets, or API composition.
- Use this when route structure, dependency behavior, status/error mapping, or generated docs affects correctness.
- If the task only changes pure service code with no HTTP surface, do not load this routing reference.

## Implementation Focus

- Organize endpoints with `APIRouter` prefixes and tags that match the accepted API contract. Do not invent versioned prefixes unless the contract owns versioning.
- Use `Annotated` dependencies for shared concerns such as database sessions, current user, pagination, filters, or settings.
- Keep endpoint functions thin: parse request data, call service/CRUD logic, map domain outcomes to HTTP responses, and return typed response models.
- Use explicit `status_code`, `response_model`, path/query constraints, and documented error responses when they are part of the contract.
- Keep list endpoints bounded with validated pagination and deterministic sorting.
- Use `HTTPException` or the repository's exception handlers for stable error mapping; do not leak raw provider/database exceptions.
- For BackgroundTasks or WebSockets, define lifecycle, idempotency, failure behavior, and test strategy before adding them.
- Keep router inclusion and dependency overrides consistent with existing project layout.

## Verification Focus

- Run endpoint tests with httpx/TestClient according to the app's sync or async style.
- Prove success, validation error, not found, conflict, unauthorized/forbidden, pagination/filtering, and response shape when touched.
- Verify OpenAPI docs or schema generation when public routes or models change.
- For dependency changes, test override behavior and cleanup in the test suite.

## Evidence Notes

- Record `fastapi.routing` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/backend/fastapi/routing.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the FastAPI routing decision: router boundary, dependency, response model, status code, pagination, error mapping, background task, WebSocket, or OpenAPI proof.
