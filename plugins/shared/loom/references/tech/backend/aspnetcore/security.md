# ASP.NET Core Security Quality

This file applies ASP.NET Core authentication, authorization, JWT, Identity, and password rules to task-owned security behavior.

## When To Use

- The task changes JWT bearer auth, ASP.NET Core Identity, authorization policies, roles/claims, password hashing, current-user access, protected route groups, CORS/security middleware, or auth error behavior.
- Use this when endpoint access, token claims, password handling, or permission behavior affects correctness.
- If the current phase explicitly has no auth work, do not add authentication because this reference is available.

## Implementation Focus

- Keep JWT issuer, audience, signing key, expiration, CORS origins, cookie settings, and security options in configuration or options binding, not source literals.
- Use ASP.NET Core Identity or a maintained password-hashing strategy; never store plaintext passwords or return password hashes.
- Configure authentication and authorization middleware in the correct order before protected endpoints.
- Prefer named authorization policies for claim/role requirements used by more than one endpoint. Keep one-off endpoint requirements local and readable.
- Protect route groups or endpoints server-side. UI hiding is not an authorization mechanism.
- Distinguish unauthenticated from unauthorized responses and keep error payloads consistent with the API contract.
- Keep user identity and ownership checks in application services when they affect business rules, not only in endpoint filters.

## Verification Focus

- Test allowed, missing token, invalid token, expired token, insufficient role/claim, wrong owner, and public endpoint behavior when touched.
- Test registration/login/password hashing/token issuance paths when auth flows change.
- Run integration tests through real middleware when verifying route protection.
- Verify configuration binding and app startup after auth service registration changes.

## Evidence Focus

- In the evidence summary, name the security decision: JWT bearer, Identity, authorization policy, role/claim check, password hashing, middleware order, ownership check, or protected endpoint proof.
