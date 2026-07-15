# Django Models, Migrations, And ORM

Django models define persisted identity, constraints, relationships, and query behavior. Migrations own schema evolution; serializers, views, and admin screens are consumers rather than substitutes for model/data integrity.

## When To Use

Use this reference for Django model fields, constraints, indexes, relationships, managers/querysets, transactions, migration operations, bulk writes, and ORM query performance. DRF-only transport changes do not need it.

## Implementation Focus

### Model And Field Semantics

Choose field types, lengths, precision, nullability, `blank`, defaults, choices, and database constraints from the accepted data model. `null` controls storage while `blank` controls validation; do not use both indiscriminately on strings.

Use stable enum values through `TextChoices`/`IntegerChoices` when values persist externally. Use `DecimalField` for money-like values and timezone-aware Django datetime behavior. Keep server-generated/default values aligned with migrations and API output.

Relationships need explicit lifecycle semantics:

- `CASCADE` only when the child cannot exist without the owner
- `PROTECT`/`RESTRICT` when deletion must be blocked
- `SET_NULL` only with nullable storage and accepted orphan meaning
- explicit through models when a many-to-many relation has attributes or lifecycle

Set useful `related_name` values and avoid accidental reverse-name collisions. Keep `__str__` bounded and free from lazy relationship queries.

### Constraints And Indexes

Database constraints are the final integrity boundary. Use `UniqueConstraint`, `CheckConstraint`, and conditional constraints where supported by the selected provider. Pre-save validation or `.exists()` can improve feedback but cannot prevent concurrent writes.

Add indexes for actual filter, join, ordering, and uniqueness paths. Prefer composite order matching common query prefixes. Do not index every field or duplicate indexes already provided by uniqueness/foreign keys without evidence.

### Managers And QuerySets

Put reusable domain query concepts in custom `QuerySet` methods and expose them through `as_manager()` or a typed manager. Querysets remain lazy; evaluate them deliberately at the owning boundary.

```python
class OrderQuerySet(models.QuerySet["Order"]):
    def visible_to(self, actor: User) -> "OrderQuerySet":
        if actor.is_staff:
            return self
        return self.filter(requester=actor)

    def for_list(self) -> "OrderQuerySet":
        return self.select_related("requester").only(
            "id", "status", "requested_at", "requester__username"
        )
```

Keep tenant/ownership scoping composable and apply it before lookup. Do not call `.all()` on an unscoped class queryset and rely on serializer/view filtering later.

### Loading And Query Shape

Use `select_related` for single-valued foreign-key/one-to-one paths and `prefetch_related`/`Prefetch` for collections and reverse relations. Match loading to list/detail serializers rather than applying a giant global prefetch.

Use `values`, `values_list`, annotations, subqueries, `Exists`, `F`, and `Q` when they express a bounded read/update efficiently. `only`/`defer` can create hidden follow-up queries if deferred fields are later accessed.

Paginate before evaluating large querysets and use deterministic ordering. Treat query-count improvements as evidence-backed changes, not assumptions.

### Transactions And Concurrency

Use `transaction.atomic()` around multi-row invariants and state transitions. Keep network calls and slow external work outside database transactions. Use `select_for_update` with explicit ordering/scope when pessimistic locking is accepted, and define behavior for lock contention.

Use `F()` expressions, constraints, or version/state predicates for concurrent counters and transitions. `save()` after a stale read is not concurrency control.

Bulk `update`, `bulk_create`, and `bulk_update` bypass `save()`, model validation, and many signals. Use them only when skipped lifecycle behavior is intentional.

### Migration Discipline

Generate and inspect migrations, then edit only when the required operation cannot be represented correctly. Use separate schema, data backfill, constraint activation, and cleanup migrations for large or compatibility-sensitive changes.

Do not run application-model code directly in data migrations; use historical models from `apps.get_model`. Keep reverse behavior and provider capabilities explicit. Never replace migrations with manual database changes or `--fake` as a routine fix.

## Verification Focus

- Test create/update/readback for changed fields, defaults, enums, decimals, timestamps, and relationships.
- Prove uniqueness/check/delete constraints and concurrent conflict behavior.
- Verify custom queryset scoping, result correctness, ordering, pagination, and query count when performance is owned.
- Exercise transaction rollback and any accepted locking/state predicate.
- Run migration checks and migrate a clean test database when schema changes.
- Use the selected provider for provider-specific constraints, indexes, JSON, locking, or SQL behavior.

## Evidence Focus

Identify the model/query/migration boundary and the persisted or query assertion proving it. Serializer validation, an in-memory model instance, or a generated migration file alone does not prove database integrity or upgrade behavior.

## Unsafe Defaults

- `null=True` and `blank=True` copied onto every optional string.
- `CASCADE` without lifecycle ownership.
- Class-level unscoped querysets for tenant-owned data.
- Unbounded relationship serialization without loader planning.
- External calls inside `transaction.atomic()`.
- Bulk operations assumed to execute `save()` or signals.
- Data migrations importing current application models.
- Manual schema drift hidden with fake migrations.
