# Django REST Framework Views And Routers

Implement accepted HTTP behavior through the smallest DRF/Django view abstraction that preserves queryset scoping, permissions, serializers, statuses, and error contracts.

## When To Use

Use this reference for DRF ViewSets, generic/API views, routers, custom actions, queryset scoping, per-action serializers/permissions, filtering, pagination, throttling integration, or Django async views.

## Implementation Focus

### Choose The View Boundary

Use `ModelViewSet` when the accepted surface genuinely owns the complete resource operation set. Use read-only viewsets or generic views for narrower resources, and `APIView`/function views for behavior that does not fit model CRUD cleanly.

Do not expose create/update/delete merely because `ModelViewSet` supplies them. Router-generated paths, names, lookup fields, and trailing slashes must match the accepted interface.

### Queryset Scoping

Scope every queryset by tenant, actor, visibility, lifecycle state, and soft-delete policy before object lookup. A class-level `queryset = Model.objects.all()` is unsafe when access varies by request.

```python
class OrderViewSet(viewsets.ModelViewSet):
    permission_classes = [IsAuthenticated, OrderPermission]
    filter_backends = [DjangoFilterBackend, OrderingFilter]
    ordering_fields = ["requested_at", "status"]

    def get_queryset(self):
        return (
            Order.objects.visible_to(self.request.user)
            .select_related("requester")
            .prefetch_related("lines")
        )
```

Object permissions run after object retrieval; they do not automatically filter list results. Apply both queryset scoping and object-level checks where required.

### Per-Action Contracts

Use explicit mappings in `get_serializer_class`, `get_permissions`, and parser/renderer behavior when actions differ. Avoid branches spread across many hooks with no visible action contract.

Server-owned fields such as actor, tenant, audit identity, and initial state are assigned from trusted request context in `perform_create` or, for multi-step behavior, an application service. Never trust them from request payloads.

### Custom Actions And State Changes

Custom `@action` endpoints need explicit detail/list scope, method, path, serializer, permissions, status, idempotency, and failure behavior. Put state transitions and multi-row writes in a transactional service rather than mutating fields directly in the view.

Return `201`/`202`/`204` only when their semantics are actually met. A `204` response has no body. Preserve location, retry, pagination, and conditional headers when declared by the API contract.

### Filtering, Search, Ordering, Pagination

Use `django-filter` or explicit filter sets for typed allowlisted filtering. Restrict search and ordering fields; never pass arbitrary client fields into ORM ordering. Keep list endpoints bounded with deterministic default ordering and maximum page size.

Match queryset loading and annotations to the selected list/detail serializer. Avoid applying expensive prefetches to actions that do not serialize those relationships.

### Exceptions And Errors

Translate expected domain/integrity failures to the accepted DRF error envelope through focused exceptions/handlers. Distinguish validation, not found, conflict, authentication, authorization, throttling, temporary dependency failure, and unexpected errors.

Do not catch `Exception` in every action or expose database/provider messages. Log unexpected failures once at a correlation-aware boundary.

### Django Async Views

Use async views only when the full I/O path benefits and supported APIs are awaited correctly. Do not wrap broad ORM workflows in repeated `sync_to_async` calls and call them concurrent by accident. Preserve thread-sensitive database behavior and transaction boundaries.

DRF support and middleware may still be synchronous depending on the repository version/configuration. Measure before converting views and keep blocking work off the event loop.

## Verification Focus

- Exercise success, validation, not-found, conflict, unauthenticated, forbidden, and ownership paths owned by the task.
- Assert queryset scoping for list, retrieve, update, delete, and custom actions.
- Verify router names/paths, lookup fields, action methods, serializers, permissions, and statuses.
- Test filters, search, ordering allowlists, pagination metadata, and empty results.
- Use query-count evidence for changed related-object loading.
- Test async views through the actual ASGI path when async behavior is owned.

## Evidence Focus

Identify the view/action, scoped queryset, permission, serializer, and HTTP assertions proving the behavior. Router registration or one successful request does not prove list isolation, object ownership, failure mapping, or query efficiency.

## Unsafe Defaults

- Full `ModelViewSet` for a read-only or single-action surface.
- Unscoped class queryset for tenant/user-owned records.
- Object permission assumed to filter list endpoints.
- State transitions and external calls directly in view methods.
- Arbitrary client-controlled ordering/filter fields.
- One expensive queryset/prefetch used for every action.
- Async views wrapped around synchronous ORM workflows without a deliberate boundary.
