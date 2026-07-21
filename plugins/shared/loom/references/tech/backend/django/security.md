# Django And DRF Security

Implement the accepted authentication and authorization model using Django/DRF's established mechanisms. Do not add SimpleJWT, registration, roles, API keys, or account workflows when the phase does not own them.

The accepted JWT algorithm, claims, and HTTP behavior live in `tech/api/jwt.md`; this file owns Django/DRF authentication classes, permission wiring, and framework-specific settings.

## When To Use

Use this reference for Django authentication, DRF authentication classes, sessions, SimpleJWT, password/account flows, permission classes, queryset/object ownership, CSRF/CORS, hosts/proxy security, or protected endpoint errors.

## Implementation Focus

### Identity Mechanism

Preserve the selected client/trust model:

| Client | Django/DRF Boundary |
|---|---|
| Same-origin browser | Django session authentication, secure cookies, CSRF |
| Bearer-token API | Trusted issuer/token validation or configured SimpleJWT contract |
| Service client | Existing API key, gateway identity, mTLS-aware, or service token boundary |
| Public endpoint | Explicit `AllowAny` only where accepted |

Keep global `DEFAULT_AUTHENTICATION_CLASSES` and `DEFAULT_PERMISSION_CLASSES` conservative. Use per-view/action overrides only where the contract differs, and make public exceptions visible and tested.

### Django Security Settings

Load secret key, allowed hosts, trusted origins, proxy SSL header, CORS origins, database credentials, and token settings from environment-aware validated configuration. Keep `DEBUG` off outside local development and never commit production credentials.

Configure secure/HTTP-only/SameSite cookies, HTTPS redirect/HSTS, and proxy trust according to the actual hosting boundary. Do not trust forwarded host/proto headers from arbitrary clients.

### Passwords And User Model

Use `set_password`, `check_password`, `create_user`, Django validators, and the selected password hashers. Never assign raw passwords to model fields or expose hashes through serializers/admin logs.

Choose a custom user model before initial migrations when the project requires one. Late replacement is a migration project, not a routine field edit. Registration, email verification, reset, lockout, and recovery each need explicit workflow ownership.

### JWT And Refresh Tokens

When the accepted profile selects JWT, map `tech/api/jwt.md` into SimpleJWT or the repository's existing verifier. Refresh, rotation, blacklist, and logout behavior remain separate capabilities and must not be added unless the API contract owns them.

### Permissions And Ownership

Use `has_permission` for operation-level access and `has_object_permission` for a retrieved object. Also scope querysets for list isolation and to avoid leaking object existence.

```python
class IsOrderOwnerOrApprover(permissions.BasePermission):
    def has_object_permission(self, request, view, order):
        if request.method in permissions.SAFE_METHODS:
            return order.requester_id == request.user.id or request.user.can_approve
        return order.requester_id == request.user.id
```

Keep tenant, ownership, and lifecycle checks in reusable policy/query/service boundaries when multiple entry points must enforce them. UI visibility and serializer field omission are not authorization.

### Sessions, CSRF, And CORS

Session-authenticated unsafe requests require CSRF protection. Do not exempt an endpoint merely because it returns JSON. Token-only clients and browser sessions may need separate routes or explicit authentication classes.

CORS permits browser origins; it is not authentication. Use explicit origins/methods/headers and never wildcard credentialed origins. Test middleware ordering and preflight for protected writes.

### Error And Data Disclosure

Preserve safe `401`/`403`/not-found behavior according to the accepted disclosure policy. Avoid account enumeration in login/reset/registration and never expose token parser, database, permission internals, or stack traces.

## Verification Focus

- Test allowed, anonymous, invalid-session/token, expired/wrong token, forbidden, wrong-owner, and admin/approver paths.
- Verify list queryset scoping independently from object permissions.
- Test password hashing and user-manager behavior without exposing credentials.
- Exercise refresh rotation/blacklist/revocation only when owned.
- Verify CSRF and CORS against the real session/bearer browser model.
- Run Django deployment/security checks when security settings change, alongside focused behavioral tests.

## Evidence Focus

Identify the authentication class, permission/ownership rule, queryset scope, and exact allowed/denied assertions. A permission class declaration or successful token creation alone does not prove endpoint protection.

## Unsafe Defaults

- Adding SimpleJWT because it is common in DRF tutorials.
- `AllowAny` or empty permission classes used to make tests pass.
- Object permission without list queryset scoping.
- Raw password assignment or password hashes in API output.
- Wildcard credentialed CORS or session writes exempted from CSRF.
- Hardcoded secret key, allowed hosts, origins, or token lifetimes.
- Trusting forwarded headers without a controlled proxy boundary.
