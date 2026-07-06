# Django Models And ORM Quality

Use this topic reference when `tech/backend/django/models.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md` and selected Python/SQL references. This file applies Django model, migration, and QuerySet rules to task-owned persistence behavior.

## When To Use

- The task changes Django models, fields, indexes, constraints, managers, QuerySets, migrations, admin-backed model behavior, transactions, or ORM performance.
- Use this when relationship loading, migration correctness, database constraints, or query shape affects the feature.
- If the task only changes DRF serializers or views with no persistence behavior, do not load this models reference.

## Implementation Focus

- Model business data with explicit field types, null/blank semantics, defaults, choices/enums, unique constraints, and database indexes for frequent filters or ordering.
- Keep data invariants in the model/service layer when they must hold outside a single serializer or view. Do not rely only on client-side checks.
- Use migrations as the schema source of truth. Run and review generated migrations; do not hand-edit database state or depend on implicit schema drift.
- Use `select_related` for foreign key/one-to-one access and `prefetch_related` for many-to-many or reverse relations. Do not hide N+1 by serializing unbounded related data.
- Put reusable filters on managers or QuerySet methods when they express domain concepts; keep ad hoc view filters local to the view.
- Use `transaction.atomic()` for multi-row writes, state transitions, and side effects that must commit together.
- Use `F`, `Q`, annotations, aggregates, bulk operations, and `only`/`defer` deliberately when they simplify query intent or performance.
- Avoid raw SQL unless the ORM cannot express the query; parameterize it and record why ORM was insufficient.

## Verification Focus

- Run Django model tests, migration checks, or pytest-django tests for changed models and managers.
- Prove create/update/read behavior, default values, constraints, indexes/ordering expectations, relationship loading, and rollback branches when relevant.
- For query changes, verify result correctness and, when performance-sensitive, query count or generated query shape.
- For migrations, verify a clean test database can migrate and the application imports models successfully.

## Evidence Notes

- Record `django.models` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/backend/django/models.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the Django ORM decision: model field, constraint, index, migration, manager/queryset, relationship loading, transaction boundary, or query optimization.
