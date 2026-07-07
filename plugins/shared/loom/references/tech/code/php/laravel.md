# PHP Laravel Quality

This file applies Laravel conventions to task-owned behavior.

## When To Use

- The task changes Laravel controllers, Form Requests, Eloquent models, migrations, resources, policies, events/listeners, jobs, queues, service providers, or feature tests.
- Use this when Laravel's request validation, authorization, ORM, serialization, or queue lifecycle affects correctness.
- If the PHP project is not Laravel, do not borrow Laravel-specific structure.

## Implementation Focus

- Keep controllers as orchestration only: authorize, validate, call an application service/action, and return a resource/response. Do not put business workflows in controllers.
- Use Form Request classes for HTTP validation and authorization when the repository follows that convention. Convert validated input into a DTO or explicit service arguments before business logic.
- Use API Resources for response shape control. Do not return raw Eloquent models from public API paths when hidden fields, casts, relations, or timestamps can leak.
- Keep Eloquent model state explicit: guarded/fillable fields, casts for enums/dates/value-like columns, relationship ownership, and soft-delete behavior when present.
- Do not add a repository layer only to wrap trivial Eloquent CRUD if the project does not already use repositories. Add one only when it owns query complexity, external persistence variation, or a clear local pattern.
- Put multi-row writes, status transitions, and side effects inside service/action transaction boundaries. Use `afterCommit` semantics for events/jobs that should not run on rolled-back data.
- Avoid N+1 query paths by selecting relation loading for the exact read case: eager load, count aggregate, scoped query, or dedicated read model. Do not globally eager-load relationships to hide a local issue.
- Use policies/gates for user authorization and keep business eligibility checks in domain/application services when they are not user permission decisions.
- Queue jobs should carry stable identifiers or serialized DTOs, not large Eloquent object graphs. Define retry, timeout, and idempotency expectations for externally visible side effects.
- Migrations should encode database constraints that protect important business invariants, not rely only on request validation.

## Verification Focus

- Run Laravel feature tests for API/controller changes and unit tests for service/action changes.
- Prove validation failure, authorization denial, successful write/read response shape, and relevant database state with assertions.
- For queues/events, use the repository's fake/test helpers to prove dispatch timing and payload, and test handler behavior when the handler owns business work.
- For Eloquent queries, test filters, pagination, sorting, relationship loading/counts, and empty/not-found behavior touched by the task.

## Evidence Focus

- In the evidence summary, name the Laravel decision: Form Request, DTO conversion, resource shape, transaction boundary, Eloquent query, policy/gate, migration constraint, job/event, or feature-test proof.
