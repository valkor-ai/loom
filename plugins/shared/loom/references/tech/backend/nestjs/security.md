# NestJS Authentication And Authorization

Implement only the accepted identity and authorization model. NestJS guards, Passport strategies, JWT helpers, and metadata are mechanisms; they do not decide whether the product uses sessions, bearer tokens, gateway identity, API keys, or another trust boundary.

The accepted JWT profile and API denial behavior live in `tech/api/jwt.md`; this file owns NestJS guards, Passport strategy wiring, and policy composition.

## Authentication Boundary

Preserve the selected mechanism and repository conventions:

| Identity source | NestJS responsibility |
|---|---|
| Same-origin session/cookie | Validate session, preserve CSRF and secure-cookie behavior |
| Bearer token | Verify trusted issuer, audience, signature, expiry, and token class |
| Gateway/service identity | Trust only validated proxy/service assertions at the configured boundary |
| Public operation | Explicit metadata/route policy, narrowly scoped |

Use Passport when its strategy lifecycle matches the application. A focused custom guard can be clearer for simple gateway or service-token validation. Do not add Passport/JWT merely because the framework supports it.

## Guard And Metadata Composition

Authentication establishes identity; authorization decides whether that identity may perform the operation. Keep these decisions separate even when guards are composed.

Read public/role/permission metadata with `Reflector.getAllAndOverride` or the established merge policy so handler-level decisions override controller defaults predictably. Use stable symbols/constants for metadata keys and keep `@Public` exceptions reviewable.

Global `APP_GUARD` providers protect by default only when every public exception is explicit. Register guards through dependency injection rather than constructing them manually in bootstrap when they need configuration or services.

```typescript
const isPublic = this.reflector.getAllAndOverride<boolean>(IS_PUBLIC_KEY, [
  context.getHandler(),
  context.getClass(),
]);
if (isPublic) return true;
return super.canActivate(context);
```

Use the repository's metadata key and precedence convention. The snippet illustrates deterministic method/controller resolution rather than prescribing JWT.

Role checks alone are insufficient for tenant, owner, relationship, or object-state authorization. Scope repository queries and enforce operation policy in the application boundary so list, detail, mutation, background, and non-HTTP entry points remain consistent.

## JWT And Token Lifecycle

Map the selected `tech/api/jwt.md` profile to validated startup configuration and the repository's Passport/resource-server strategy. Never accept an algorithm or key from untrusted token headers without policy.

Keep access tokens short-lived according to the accepted security contract. Refresh tokens need explicit rotation, reuse detection, storage/hashing, revocation, device/session handling, and logout semantics. Do not issue a long-lived signed token and call it a refresh workflow.

Claims should contain stable authorization inputs, not sensitive profile data or mutable facts assumed to stay current indefinitely. Validate subject/account state where the contract requires revocation or suspension to take effect promptly.

## Password And Account Flows

Use the established password hasher with configured cost and constant-time verification behavior. Never store or return plaintext passwords, hashes, reset tokens, verification tokens, or refresh tokens through ordinary user DTOs.

Registration, login, verification, reset, recovery, lockout, and MFA are separate workflows. Implement only task-owned flows, use single-use/expiring secrets where required, and avoid account enumeration in responses and timing where practical.

## Browser And Cross-Origin Controls

Session/cookie authentication requires CSRF protection for unsafe methods. CORS controls browser origins; it is not authorization. Configure explicit origins, credentials, methods, and headers, and ensure proxy/HTTPS/cookie settings match the real deployment boundary.

Bearer tokens in browser storage have a different threat model. Do not switch storage or disable CSRF/CORS protections as a local workaround for integration failures.

## Errors, Logs, And Secrets

Preserve `401` for missing/invalid authentication and `403` for authenticated-but-disallowed operations according to the accepted disclosure policy. Wrong-owner lookup may intentionally map to not found; apply that policy consistently.

Use `ConfigModule` or the repository's typed configuration boundary for secrets, lifetimes, issuer/audience, cookie, and hashing settings. Redact credentials and tokens from logs, traces, exception metadata, and Swagger examples.

## Verification

- Exercise allowed, missing-identity, invalid/expired token, insufficient-permission, wrong-owner/tenant, and explicit-public paths.
- Run protected operations through the real Nest application so global guards and metadata resolution execute.
- Test issuer/audience/algorithm and refresh rotation/revocation only when those token behaviors are owned.
- Verify password hashing and ensure every response/serialization path excludes secrets.
- Test list isolation separately from detail and mutation authorization.
- Exercise CSRF, CORS preflight, secure-cookie, or proxy trust only for the selected browser/deployment model.

## Delivery Evidence

Identify the identity mechanism, guard/policy boundary, protected operation, and both allowed and denied assertions. A mocked guard or decoded token alone cannot prove global registration, metadata precedence, ownership scoping, revocation, or response redaction.

## Unsafe Defaults

- JWT/Passport added without an accepted identity contract.
- `@Public` metadata checked only on the handler, ignoring controller precedence.
- Roles treated as row-level authorization.
- Long-lived refresh tokens with no rotation or revocation model.
- Secrets read through scattered `process.env` calls.
- CORS or CSRF disabled to make browser requests pass.
- Password/token fields omitted from one DTO but leaked through another serializer.
