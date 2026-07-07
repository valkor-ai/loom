# Express To NestJS Migration Quality

This file applies migration rules when a task explicitly ports Express behavior into NestJS.

## When To Use

- The task explicitly migrates Express routes, routers, middleware, validation, services, auth middleware, error handlers, or tests into NestJS modules/controllers/providers.
- Use this when preserving existing behavior matters more than designing a new NestJS API from scratch.
- Do not load this file for ordinary NestJS feature work that is not an Express migration.

## Implementation Focus

- Preserve accepted behavior first: route paths, status codes, validation messages, auth behavior, pagination/filtering, and response shape should only change when the task says so.
- Map Express routers to controllers and modules; map handler functions to controller methods plus service calls.
- Replace manual request parsing with decorators, DTOs, pipes, and explicit parameter binding.
- Replace middleware-as-business-logic with guards, pipes, interceptors, filters, or services according to responsibility.
- Replace manual service construction with Nest providers and constructor injection.
- Preserve existing error semantics through Nest exceptions or exception filters.
- Migrate tests before or alongside behavior so parity failures are visible.

## Verification Focus

- Compare old and new behavior for at least one success case and one failure/auth case per migrated route group.
- Test route binding, validation, auth/role behavior, error payloads, and response shape parity.
- Verify module compilation and provider injection for migrated services.
- Record intentional behavior differences as known gaps rather than silently changing the contract.

## Evidence Focus

- In the evidence summary, name the migration decision: router-to-controller mapping, middleware-to-guard/pipe/filter mapping, DI conversion, validation parity, error parity, test parity, or accepted behavior difference.
