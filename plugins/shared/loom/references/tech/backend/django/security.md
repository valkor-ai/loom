# Django Security Quality

This file applies Django, DRF, and SimpleJWT security rules to task-owned authentication and authorization behavior.

## When To Use

- The task changes Django authentication, DRF authentication classes, SimpleJWT, permissions, object permissions, CSRF/CORS behavior, registration/login/current-user endpoints, or protected view behavior.
- Use this when endpoint access, token behavior, password handling, or user ownership affects correctness.
- If the current phase explicitly has no auth work, do not add authentication just because this reference is available.

## Implementation Focus

- Keep secrets, signing keys, DEBUG, allowed hosts, CORS origins, database credentials, and token lifetimes in environment-aware settings. Do not commit production secrets in `settings.py`.
- Use Django's password hashing and user model APIs. Never store plaintext passwords or compare raw password strings.
- Configure DRF authentication classes and permission classes at the narrowest appropriate scope: global defaults for platform policy, per-view/per-action overrides for route-specific behavior.
- Use object-level permissions or queryset scoping for ownership and tenant boundaries. A class-level permission alone is not enough for row-level protection.
- For JWT, configure access/refresh lifetimes, rotation/blacklist policy, auth header type, and custom claims deliberately.
- Preserve CSRF protection for session-auth browser flows; disable or exempt only when the API style and threat model justify it.
- Map unauthenticated to 401 and unauthorized to 403 according to the repository's existing error style, without leaking sensitive resource existence.
- Keep registration, login, refresh, current-user, and password flows separated from business resources unless the task owns that workflow.

## Verification Focus

- Test allowed, unauthenticated, unauthorized, wrong-owner, and admin-only paths for protected endpoints.
- Test token obtain, refresh, invalid token, expired token, custom claim, and blacklist behavior when touched.
- Test password hashing and registration validation for auth flows.
- Run security-related API tests with real DRF permission evaluation rather than bypassing all auth through mocks.

## Evidence Focus

- In the evidence summary, name the Django security decision: auth class, permission, object permission, JWT setting, password handling, CSRF/CORS behavior, ownership check, or protected endpoint proof.
