# NestJS Service And DI Quality

This file applies NestJS provider, module, service, dependency injection, and business orchestration rules to task-owned backend code.

## When To Use

- The task changes NestJS services, providers, modules, exports/imports, custom providers, repository adapters, transactions, exception mapping, interceptors, or dependency injection wiring.
- Use this when service ownership, module boundary, provider lifetime, or dependency graph affects correctness.
- If the task only changes DTO validation or controller metadata, do not load this service reference unless a service change is also required.

## Implementation Focus

- Use constructor injection and `@Injectable()` providers. Do not instantiate services manually with `new` inside business code.
- Keep module imports/exports explicit. Export only providers required by other modules and avoid circular dependencies; use `forwardRef` only as a last resort.
- Put business rules, repository calls, transactions, and external integration orchestration in services or dedicated providers, not controllers.
- Throw typed Nest exceptions or domain-specific errors that the repository's exception layer maps consistently.
- Keep provider tokens and factory providers explicit when injecting config, clients, or non-class dependencies.
- Use request-scoped providers only when per-request state is genuinely required.
- Keep persistence-specific implementation behind the repository/ORM convention already present, whether TypeORM, Prisma, or another adapter.

## Verification Focus

- Run service unit tests with `Test.createTestingModule` and mocked owned boundaries.
- Verify module compilation/DI graph when imports, exports, providers, or injection tokens change.
- Test success, not found, conflict, validation-driven service rejection, transaction behavior, and external-boundary failures when touched.
- Run lint/typecheck to catch circular imports, missing providers, and type leaks.

## Evidence Focus

- In the evidence summary, name the NestJS service decision: provider boundary, module export, DI token, business rule, transaction, repository adapter, typed exception, or module compilation proof.
