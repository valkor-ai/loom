# ASP.NET Core Authentication And Authorization

Implement the accepted identity and authorization model. ASP.NET Core supports cookies, bearer tokens, Identity, external providers, certificates, and custom schemes; do not introduce JWT or Identity unless the architecture and client trust model select them.

## Scheme Selection And Configuration

Configure explicit default authenticate/challenge/forbid schemes when more than one scheme exists. Keep cookie and bearer endpoints separated through policy schemes or endpoint metadata where needed; do not let scheme forwarding accept weaker credentials accidentally.

Bind issuer, audience, authority, keys, lifetimes, cookie settings, and provider credentials through validated options. Never commit signing secrets or log tokens. Place `UseAuthentication` before `UseAuthorization` and map protected endpoints after both.

For JWT bearer validation, pin trusted issuer, audience, signing algorithms/keys, lifetime, and clock skew. Define key rotation and authority availability behavior. Do not generate local self-issued tokens when the accepted system trusts an external identity provider.

## Identity And Password Workflows

Use ASP.NET Core Identity or the existing maintained credential boundary for password hashing, lockout, confirmation, reset, MFA, recovery, and security-stamp behavior. Do not implement a custom PBKDF/password format from a tutorial when Identity already owns credentials.

Registration, login, verification, reset, and account recovery are separate task-owned workflows. Avoid account enumeration, use expiring single-use tokens where required, and never return password hashes, reset tokens, refresh tokens, or security stamps in ordinary DTOs.

Refresh/session workflows need explicit rotation, revocation, reuse detection, device/session ownership, and logout semantics. A long-lived bearer token is not a complete refresh design.

## Policies And Resource Authorization

Use named policies for reusable claim/role/requirement combinations. Prefer custom `IAuthorizationRequirement` handlers for policy that depends on application facts rather than branching across endpoint delegates.

```csharp
builder.Services.AddAuthorization(options =>
{
    options.AddPolicy("CanApproveOrders", policy =>
        policy.RequireAuthenticatedUser()
              .AddRequirements(new OrderApprovalRequirement()));
});
```

Roles and claims establish coarse capabilities; they do not prove row/tenant ownership or valid lifecycle state. Scope list queries and enforce resource/operation authorization in the application boundary. Use `IAuthorizationService.AuthorizeAsync(user, resource, policy)` when a loaded resource is required.

Keep fallback/default policy behavior explicit. Anonymous endpoints must be deliberately marked and reviewed, especially when a route group has shared authorization metadata.

## Current User Boundary

Read identity through a narrow request-context abstraction or endpoint `ClaimsPrincipal`, then pass stable actor/tenant identifiers into application operations. Domain and infrastructure code should not depend directly on `IHttpContextAccessor` unless request context is their accepted responsibility.

Validate claim presence and format; never assume `sub`, tenant, role, or email claims exist because one issuer usually supplies them. Map external identity to internal account state where suspension/revocation requires current data.

## Browser Controls

Cookie-authenticated unsafe requests require antiforgery protection. Configure secure, HTTP-only, SameSite, domain/path, and expiration behavior for the real deployment topology.

CORS controls browser origin access and is not authentication. Use explicit origins/methods/headers and never wildcard credentialed origins. Preserve forwarded-header/proxy trust so HTTPS and secure-cookie decisions cannot be spoofed.

Do not move bearer tokens to insecure browser storage or disable antiforgery/CORS to work around frontend integration problems.

## Error And Disclosure Policy

Preserve `401` challenge versus `403` forbid behavior and the accepted problem envelope. Wrong-owner resources may intentionally return not found; apply that policy consistently across list/detail/mutation paths.

Redact credentials, tokens, claims not needed for diagnosis, cookies, and authorization internals from logs/traces. Production errors must not expose validation keys, cryptographic details, database messages, or stack traces.

## Verification

- Exercise allowed, unauthenticated, invalid/expired token, forbidden policy, wrong-owner/tenant, and explicit anonymous paths through the real middleware pipeline.
- Test list isolation separately from resource authorization and mutation policy.
- Verify issuer/audience/key/lifetime handling and refresh/revocation only when those workflows are owned.
- Assert password hashing and secret-field exclusion through real response serialization.
- Test cookie/antiforgery/CORS/proxy behavior for the selected browser topology.
- Validate startup failure for missing mandatory auth options without exposing secrets.

## Delivery Evidence

Name the selected scheme, policy/resource boundary, protected operation, and allowed plus denied HTTP assertions. A decoded token, mocked `ClaimsPrincipal`, or endpoint metadata inspection alone cannot prove middleware order, scheme selection, ownership isolation, revocation, or secret redaction.

## Unsafe Defaults

- JWT or Identity added because a .NET example uses it.
- Signing keys and token options read from unvalidated literals.
- Roles treated as object/tenant authorization.
- `IHttpContextAccessor` injected throughout domain/application code.
- Refresh tokens issued without storage, rotation, or revocation policy.
- Cookie antiforgery or CORS disabled to make integration pass.
- Anonymous endpoint exceptions hidden inside broad route groups.
