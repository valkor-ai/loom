# Django To FastAPI Migration Quality

This file applies migration rules when a task explicitly ports behavior from Django/DRF to FastAPI.

## When To Use

- The task explicitly migrates or rewrites Django/DRF models, serializers, viewsets, permissions, filters, or tests into FastAPI/Pydantic/SQLAlchemy.
- Use this when preserving existing behavior matters more than designing a new API from scratch.
- Do not load this file for ordinary FastAPI feature work that is not a Django/DRF migration.

## Implementation Focus

- Preserve accepted behavior first: routes, status codes, validation errors, permissions, filtering, pagination, and response shape should only change when the task says so.
- Map DRF serializers to Pydantic create/read/update schemas; keep read-only/write-only/sensitive-field behavior explicit.
- Map ViewSets and custom actions to APIRouter endpoints with explicit dependencies, status codes, and service calls.
- Map DRF permissions and object ownership to FastAPI dependencies and service-level checks. Do not lose row-level protection during the rewrite.
- Separate ORM models from Pydantic schemas. Do not treat Pydantic models as database entities.
- Recreate query optimization deliberately: `select_related`/`prefetch_related` intent should become SQLAlchemy eager loading or explicit query shape.
- Treat Django admin, forms, templates, signals, and middleware as migration risks; keep them out of scope unless the task owns them.
- Migrate tests before or alongside behavior so parity failures are visible.

## Verification Focus

- Compare old and new behavior for at least one success case and one failure/permission case per migrated endpoint.
- Test validation, pagination/filtering, ownership, auth denial, not-found, conflict, and response shape parity when touched.
- Verify database migration or data-access parity for relationships and constraints that existed in Django.
- Record intentional behavior differences as known gaps rather than silently changing the contract.

## Evidence Focus

- In the evidence summary, name the migration decision: serializer-to-schema mapping, ViewSet-to-router mapping, permission parity, ORM query parity, test parity, or accepted behavior difference.
