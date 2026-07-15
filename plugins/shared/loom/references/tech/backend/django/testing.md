# Django And DRF Testing

Use the narrowest Django/DRF test boundary that proves task-owned behavior. This reference is selected only for tasks that explicitly own test implementation.

## When To Use

Use this reference for Django model/query/migration, serializer, view/router, permission, middleware, settings, admin, management-command, or DRF API tests. Pure Python rules remain in the Python testing reference.

## Implementation Focus

### Test Boundary Matrix

| Behavior | Suitable Boundary |
|---|---|
| Pure domain/service rule | Plain pytest/unittest without Django setup where possible |
| Model field/constraint/manager | `TestCase` or pytest-django database test |
| Transaction/commit/locking | `TransactionTestCase` or transaction-enabled pytest test |
| Serializer validation/shape | Focused serializer test |
| DRF route/status/permission | `APITestCase`, `APIClient`, or pytest-django API client |
| Middleware/settings | RequestFactory/client plus `override_settings` |
| Migration/data migration | Migration executor or repository migration-test helper |
| Query optimization | Result assertions plus query-count capture |

Do not boot the full API stack for a pure calculation. Do not test a protected endpoint only after bypassing every authentication/permission component.

### Data Factories And Isolation

Prefer factories/builders with scenario-relevant fields and safe defaults. Keep mutable data per test and avoid giant shared fixtures that obscure ownership or create ordering dependencies.

Use `setUpTestData` for immutable class data only when mutation cannot leak between tests. Use timezone-aware fixed times and deterministic IDs/randomness where outcomes depend on them.

Database access must be explicit in pytest-django. Do not mark every test with database access by default. Use the selected provider for dialect, locking, JSON, constraint, index, or migration behavior that differs from SQLite.

### API And Permission Tests

Use named routes with `reverse` when route names are part of repository convention. Assert exact status, body/error shape, headers, pagination, and database side effects.

Use `force_authenticate` only when token/session parsing is not under test. Security tests should obtain or construct credentials through the real selected mechanism.

Every changed protected operation should include allowed and denied cases, including anonymous, insufficient permission, and wrong owner/tenant where applicable. Test list isolation separately from detail object permissions.

```python
class OrderAccessTests(APITestCase):
    def test_other_requester_cannot_retrieve_order(self):
        self.client.force_authenticate(self.other_user)
        response = self.client.get(reverse("order-detail", args=[self.order.pk]))
        self.assertEqual(response.status_code, status.HTTP_404_NOT_FOUND)
```

### Transactions And Commit Behavior

`TestCase` wraps tests in transactions and may hide real commit callbacks or lock behavior. Use `captureOnCommitCallbacks(execute=True)`, `TransactionTestCase`, or an integration boundary when post-commit events, another connection, constraints, or locking matter.

Assert rollback and durable readback rather than only in-memory model state. Avoid catching broad exceptions in tests; assert the expected constraint or application error.

### Query And Serialization Evidence

For N+1 or query optimization work, assert correct results and query count through `assertNumQueries` or query capture. Stabilize data volume and serializer/view path so the count proves the intended access pattern.

Do not overfit exact SQL text unless provider-specific SQL is the contract. Query count alone does not prove correct scoping or response shape.

### Migration Tests

Run migration graph/check commands for model changes. Test data migrations with representative old-state rows and assert the upgraded state using historical models. Verify reverse migration only when reversibility is claimed.

Do not disable migrations in the only verification for schema changes.

### Async Views And External Boundaries

Use Django's async test support/ASGI path for async view behavior and keep sync ORM boundaries deliberate. Mock external ports at the adapter boundary; use protocol fakes when serialization/status/error translation is under test.

Avoid arbitrary sleeps and shared network services. Use completion signals or repository-standard workers for accepted background behavior.

## Verification Focus

- Run the changed test module/class first, then the owning Django app suite when shared model/router/settings behavior changes.
- Cover success plus validation, not-found, conflict, auth, ownership, and rollback paths relevant to the task.
- Prove persisted side effects and absence of forbidden side effects.
- Verify query loading/count for claimed ORM improvements.
- Run migration checks or focused migration tests for schema/data changes.
- Ensure overrides, settings, credentials, files, cache, mail, and global state are restored.

## Evidence Focus

Record the test boundary, scenario, command, and meaningful assertion. A passing suite or model factory creation alone does not prove route protection, migration correctness, transaction timing, or query efficiency.

## Unsafe Defaults

- Full-stack API tests for every pure Python branch.
- `force_authenticate` in the only authentication test.
- Shared mutable fixtures or test-order dependencies.
- SQLite claimed as proof for provider-specific behavior.
- `TestCase` used to prove real commit/locking behavior without adjustment.
- Query-count assertions without result/scoping assertions.
- Migrations disabled for model-change verification.
- Arbitrary sleeps or uncleaned settings/dependency overrides.
