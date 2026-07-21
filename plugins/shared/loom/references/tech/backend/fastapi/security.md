# FastAPI Authentication And Authorization

Implement only the accepted identity and access policy. FastAPI dependencies are transport/application guards; they do not justify adding JWT, password login, roles, refresh tokens, or account storage to an unauthenticated phase.

The accepted JWT profile and API behavior live in `tech/api/jwt.md`. This file owns FastAPI dependency, middleware, and configuration wiring.

## When To Use

Use this reference when a task owns authentication extraction, token/session validation, current-actor dependencies, permissions, resource ownership, password handling, security middleware, or protected route behavior.

## Implementation Focus

### Mechanism Boundary

Preserve the repository and accepted trust model:

| Client/Trust Model | Suitable FastAPI Boundary |
|---|---|
| External bearer-token API | OAuth2 bearer extraction plus issuer/JWK or trusted JWT validation |
| Same-origin browser session | Secure session cookie, CSRF protection, and current-actor dependency |
| Service-to-service | Existing gateway identity, mTLS-aware infrastructure, or service token validation |
| Explicitly public/internal endpoint | No placeholder login system; declare the route public |

`OAuth2PasswordBearer` extracts a bearer token and documents the scheme; it does not validate signatures, claims, users, or permissions. Prefer a maintained issuer/JWK validation path over handwritten JWT parsing when an identity provider owns tokens.

### Token Validation Wiring

Implement the claims, algorithm, issuer, audience, expiry, not-before, subject, token type, key source, and clock-skew requirements from `tech/api/jwt.md`. The FastAPI layer must pass those settings to a maintained verifier and must not add a second token contract.

```python
oauth2_scheme = OAuth2PasswordBearer(tokenUrl="/auth/token")

async def require_actor(
    token: Annotated[str, Depends(oauth2_scheme)],
    users: Annotated[UserRepository, Depends(get_user_repository)],
) -> Actor:
    claims = token_verifier.verify_access_token(token)
    actor = await users.find_actor(claims.subject)
    if actor is None or not actor.active:
        raise unauthenticated()
    return actor
```

Use a stable safe `401` response with `WWW-Authenticate: Bearer` for missing or invalid bearer credentials. Do not reveal whether an account exists or which token check failed.

### Passwords And Login

Use a maintained adaptive password-hashing library and repository-approved parameters. Never store plaintext or reversible passwords, compare raw hashes manually, or serialize password hashes.

Login endpoints need rate/abuse handling only when accepted by the API/security contract. Use a generic invalid-credentials result and avoid account enumeration. Password reset, verification, lockout, and recovery are separate capabilities, not implied by login.

### Authorization And Ownership

Use typed dependencies for coarse endpoint permissions and application services for resource ownership and lifecycle eligibility. A role check alone does not prove access to a particular record.

```python
def require_permissions(*required: Permission):
    async def dependency(actor: CurrentActor) -> Actor:
        if not set(required).issubset(actor.permissions):
            raise HTTPException(status_code=403, detail="Forbidden")
        return actor
    return dependency
```

Keep `401` for unauthenticated callers and `403` for authenticated callers lacking permission. Hide or expose protected-resource existence according to the accepted policy. UI visibility is never authorization.

### Refresh, Revocation, And Logout

Add refresh tokens only when the accepted contract requires them. Enforce a distinct token type, audience/scope, lifetime, rotation or reuse policy, storage/revocation model, and theft response. Never accept a refresh token at an access-token dependency.

Deleting a browser/client token does not revoke a stateless server token. Logout behavior must match the actual session, denylist, rotation, or short-lived-token model.

### Cookies, CORS, And CSRF

Cookie-authenticated browsers require secure, HTTP-only, appropriate `SameSite` cookies and CSRF protection for state-changing requests. Bearer-token APIs should not disable unrelated browser protections by habit.

Use explicit CORS origins, methods, and headers. Wildcard origins cannot be combined safely with credentialed browser requests. Keep CORS and authentication middleware ordering verified.

### Sensitive Data And Errors

Never log authorization headers, bearer/refresh tokens, passwords, signing keys, raw credential payloads, or sensitive claims. Public errors remain stable; the selected application observability reference owns protected correlation-aware diagnostics.

## Verification Focus

- Test allowed, missing-auth, invalid-token/session, expired, wrong issuer/audience/type, and insufficient-permission paths owned by the task.
- Prove resource-ownership denial independently from broad roles.
- Verify `401`, `403`, `WWW-Authenticate`, and accepted safe error bodies.
- Test password hashing/verification without exposing credentials or hashes.
- Test refresh rotation/reuse/revocation only when that capability exists.
- Verify cookie, CSRF, and CORS behavior against the real browser trust model.

## Evidence Focus

Identify the protected operation, identity mechanism, claim/permission rule, and exact denial/success assertions. A dependency appearing in a function signature or an OpenAPI lock icon does not prove authentication or authorization behavior.

## Unsafe Defaults

- Adding custom JWT login because FastAPI examples use it.
- Hardcoded secrets, algorithms, issuers, audiences, lifetimes, or origins.
- Treating token extraction as token validation.
- Reusing refresh tokens as access tokens.
- Role checks without resource ownership checks.
- `401` and `403` collapsed into one ambiguous result.
- Wildcard credentialed CORS or cookie auth without CSRF protection.
- Tokens, passwords, hashes, or sensitive claims in logs or response models.
