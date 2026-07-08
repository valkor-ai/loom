# NestJS Security Quality

This file applies NestJS JWT, Passport, guards, roles, and auth-module rules to task-owned security behavior.

## When To Use

- The task changes Passport strategies, JWT module setup, auth service, guards, role decorators, public-route metadata, password hashing, protected controllers, or auth error behavior.
- Use this when endpoint access, token claims, current-user identity, or permission behavior affects correctness.
- If the current phase explicitly has no auth work, do not add auth because this reference is available.

## Implementation Focus

- Keep JWT secrets, token lifetimes, issuer/audience, bcrypt cost, and auth-related config in ConfigModule or existing configuration, not source literals.
- Use Passport strategies and guards for authentication; use roles/permission guards or service checks for authorization.
- Keep `@Public` metadata, global guards, route-level guards, and role decorators consistent so public/private intent is visible and testable.
- Hash passwords before storage and never return password hashes or refresh tokens in unintended response DTOs.
- Distinguish unauthorized from forbidden behavior and keep exception payloads consistent with the API contract.
- Keep auth module dependencies explicit; avoid circular imports between AuthModule and feature modules.
- Put ownership and tenant checks in services/guards where they apply to the protected operation.

## Verification Focus

- Test login, registration, token validation, missing token, invalid token, expired token, wrong role, wrong owner, and public-route bypass when touched.
- Verify guards run in E2E tests for protected controllers rather than only mocking service methods.
- Test password hashing and sensitive field exclusion.
- Run module compilation tests after strategy, guard, or AuthModule provider changes.

## Evidence Focus

- In the evidence summary, name the NestJS security decision: JWT strategy, AuthGuard, RolesGuard, public-route metadata, auth service, password hashing, module dependency, or protected route proof.
