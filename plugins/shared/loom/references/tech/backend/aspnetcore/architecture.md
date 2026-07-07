# ASP.NET Core Clean Architecture Quality

This file applies Clean Architecture, CQRS, MediatR, dependency inversion, and layer-boundary rules to task-owned .NET code.

## When To Use

- The task changes project structure, Domain/Application/Infrastructure/WebApi boundaries, CQRS command/query handlers, MediatR pipelines, application interfaces, or dependency injection composition.
- Use this when layer ownership, use-case orchestration, or dependency direction affects correctness.
- If the task only adds a simple endpoint or persistence mapping inside an existing convention, do not load this architecture reference unless the task changes those boundaries.

## Implementation Focus

- Keep domain models and domain exceptions independent from ASP.NET Core, EF Core infrastructure, HTTP DTOs, and external clients.
- Put use-case orchestration in Application handlers/services. Keep WebApi endpoints thin and Infrastructure implementations behind interfaces.
- Use dependency inversion for persistence, identity, clock, external services, and messaging boundaries. Register concrete adapters in the composition root.
- Use CQRS/MediatR only when the repository already uses it or the task owns a feature slice that benefits from explicit command/query separation.
- Keep validation responsibilities clear: syntax/request validation at the API boundary, use-case validation in Application, invariants in Domain.
- Avoid circular project references and cross-layer shortcuts. Infrastructure may depend inward; Domain must not depend outward.
- Keep transactions and side effects owned by the use case, not scattered across endpoint code and repositories.

## Verification Focus

- Run build/tests for affected projects so broken project references, DI registrations, and handler discovery are caught.
- Test command/query handlers directly for business behavior and through endpoints when HTTP mapping is in scope.
- Verify DI composition and pipeline behavior when adding handlers, validators, or infrastructure adapters.
- Record any intentional deviation from existing layer convention as a known gap.

## Evidence Focus

- In the evidence summary, name the architecture decision: layer boundary, CQRS handler, MediatR pipeline, dependency inversion, use-case service, transaction ownership, or DI composition proof.
