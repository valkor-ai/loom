# Django REST Framework View Quality

Use this topic reference when `tech/backend/django/views.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`, selected serializer references, and selected API references. This file applies DRF viewset, generic view, router, and Django async view rules to task-owned HTTP behavior.

## When To Use

- The task changes DRF ViewSets, generic views, routers, custom actions, permissions per action, pagination, filtering, ordering, search, or Django async views.
- Use this when endpoint behavior, queryset scoping, status codes, or route registration affects correctness.
- If the task only changes internal model/service code with no HTTP surface, do not load this views reference.

## Implementation Focus

- Choose the smallest DRF abstraction that fits the endpoint: ViewSet for resource sets, generic views for simple list/detail flows, function/class views only when they better match the route.
- Keep `get_queryset()` scoped by user, tenant, status, and action. Do not expose all rows from a class-level queryset when access depends on the request.
- Use `get_serializer_class()` and `get_permissions()` when actions need different input/output or permission rules; keep the mapping explicit.
- Put write ownership in `perform_create`/`perform_update` or a service method. Do not trust request payloads for server-owned fields.
- Use filter backends, pagination, search, ordering, and lookup fields according to the accepted API contract; keep list endpoints bounded and deterministic.
- For custom `@action` endpoints, define detail/list scope, method, status code, serializer, permission, and side effect clearly.
- For Django async views, wrap synchronous ORM access with the proper sync boundary and avoid pretending Django ORM calls are natively async.
- Keep exception and error payload behavior aligned with the repository's API error style.

## Verification Focus

- Run APITestCase, APIClient, pytest-django, or DRF view tests for success, validation error, not found, permission denial, pagination/filtering, and custom actions.
- Test queryset scoping so users cannot see or mutate rows outside their ownership or tenant boundary.
- Verify route names and router registration when URLs are added or changed.
- For async views, test the async path and verify synchronous ORM work is isolated correctly.

## Evidence Notes

- Record `django.views` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/backend/django/views.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the DRF view decision: ViewSet/generic view, queryset scoping, action serializer, action permission, pagination/filtering, custom action, route registration, or async view boundary.
