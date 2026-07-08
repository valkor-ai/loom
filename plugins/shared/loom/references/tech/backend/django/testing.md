# Django Testing Quality

This file applies Django, DRF, and pytest-django test rules to task-owned changes.

## When To Use

- The task changes Django models, migrations, managers, serializers, views, permissions, settings, admin behavior, or tests.
- Use this when TestCase, APITestCase, APIClient, pytest-django, factories, fixtures, query-count checks, or migration checks are needed to prove behavior.
- If the task changes only pure Python code outside Django wiring, use Python testing references without this Django testing reference.

## Implementation Focus

- Choose the test level that proves the change: model tests for invariants/managers, serializer tests for validation/shape, API tests for routing/permissions/status codes, and settings/import tests for configuration.
- Use factories or clear fixtures for test data; avoid order-dependent tests and shared mutable database state.
- Test both authenticated and unauthenticated paths when endpoint access changes. Use `force_authenticate` only when the test is not about token parsing.
- Prefer `reverse()` or named routes when the project already uses route names; keep hardcoded paths only when that is the local convention.
- Use query-count assertions when the task claims ORM optimization or changes related-object loading.
- Keep migrations runnable in tests. Do not skip migrations to hide broken model changes unless the repository already has a deliberate test strategy for that.

## Verification Focus

- Run the repository's Django test command, such as `python manage.py test`, `pytest`, or the project-specific target.
- Prove status codes and response bodies for success, validation error, not found, permission denial, and ownership boundaries when touched.
- Verify database side effects for create/update/delete flows, including defaults, constraints, and rollback behavior.
- For JWT/auth changes, test token obtain/refresh/protected endpoint behavior with real auth components.

## Evidence Focus

- In the evidence summary, name the proof type: model test, serializer test, APITestCase, pytest-django, factory/fixture, auth test, query-count check, or migration check.
