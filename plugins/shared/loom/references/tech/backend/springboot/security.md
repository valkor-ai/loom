# Spring Boot Security Implementation

Implement the authentication and authorization policy already owned by the current interfaces and architecture. Do not introduce JWT, OAuth2, sessions, roles, refresh tokens, or account management when protected operations are not part of the task.

## Security Mechanism Boundary

Choose the mechanism from the accepted baseline and existing repository:

| Client/Trust Model | Typical Spring Security Shape |
|---|---|
| Same-origin browser session | Session authentication, secure cookies, CSRF protection |
| Stateless bearer-token API | OAuth2 Resource Server with issuer/JWK validation |
| Service-to-service API | Resource Server, mTLS-aware infrastructure, or existing gateway identity |
| Internal unauthenticated runtime | Explicit public routes only when accepted; no placeholder auth system |

Prefer Spring Security's OAuth2 Resource Server support over a custom JWT parsing filter when issuer/JWK or signed bearer tokens are the requirement. A custom filter is justified only by an existing token contract that Resource Server cannot represent.

## Filter Chain

Use Spring Security 6 `SecurityFilterChain` and explicit route ownership.

```java
@Bean
SecurityFilterChain apiSecurity(HttpSecurity http) throws Exception {
    return http
        .securityMatcher("/api/**")
        .authorizeHttpRequests(authorize -> authorize
            .requestMatchers(HttpMethod.GET, "/api/catalog/**").hasAuthority("catalog:read")
            .requestMatchers(HttpMethod.POST, "/api/orders/**").hasAuthority("orders:write")
            .anyRequest().authenticated())
        .oauth2ResourceServer(oauth -> oauth.jwt(Customizer.withDefaults()))
        .build();
}
```

Keep matcher order from specific to general. Avoid broad `permitAll`, accidental catch-all protection that blocks health/static assets, and parallel filter chains with overlapping matchers. Preserve the established role-versus-authority convention.

Do not disable CSRF by habit. Stateless bearer-token APIs that do not authenticate with cookies can disable it deliberately. Browser sessions and cookie-based authentication require CSRF protection. Document the actual client mechanism in configuration or tests.

## Authentication Material

- Store passwords with the repository's selected adaptive encoder; never plaintext or reversible encryption.
- Externalize issuer, audience, JWK location, client credentials, signing material, token lifetime, and allowed clock skew.
- Validate issuer, audience, expiry, signature, and token type required by the contract.
- Never log bearer tokens, refresh tokens, passwords, authorization headers, or decoded sensitive claims.
- Keep production credentials out of default application configuration; placeholders and environment bindings are allowed.

Custom refresh-token flows require independent token type, persistence or revocation semantics, rotation/reuse detection, expiry, logout behavior, and theft response. Clearing `SecurityContextHolder` does not revoke a stateless token.

## Authorization Boundary

Route authorization protects endpoint classes. Method authorization protects business operations reachable from multiple entry points.

```java
@PreAuthorize("hasAuthority('orders:approve') and @orderAccess.canApprove(#orderId, authentication)")
public OrderResponse approve(UUID orderId) {
    return orders.approve(orderId);
}
```

Keep resource ownership and lifecycle eligibility in an authorization/domain service rather than embedding repository queries in SpEL. UI visibility is not authorization. Controller checks alone are insufficient when jobs, messages, or other services can call the same operation.

Differentiate:

- missing or invalid authentication: `401`
- authenticated caller lacking permission: `403`
- protected resource existence: hide or expose according to accepted policy
- business ineligibility after authorization: domain conflict or validation response

Use `AuthenticationEntryPoint` and `AccessDeniedHandler` to produce the accepted safe error envelope. Do not expose parser exceptions, account existence, internal role mappings, or stack traces.

## Current User Resolution

Prefer an immutable application-facing principal containing only stable identity and authorities. Do not pass `HttpServletRequest` or Spring Security internals through domain code. Load current mutable user state only when the operation requires it.

For reactive applications, use reactive security context APIs; ThreadLocal assumptions do not cross reactive boundaries safely.

## CORS And Security

Share one explicit CORS policy with the web boundary. Credentialed browser requests require explicit origins. Preflight must reach the CORS/security filters without accidentally permitting the protected operation itself.

## Verification Focus

Useful security evidence includes:

- filter-chain context startup
- one allowed request and one denied request for every changed protected policy
- missing, malformed, expired, wrong-issuer/audience, and insufficient-authority token behavior when owned
- method-level resource ownership denial
- password hashing and invalid-credential behavior without secret disclosure
- CSRF behavior matching session or bearer-token client style
- CORS preflight for the real browser origin boundary
- stable `401` and `403` response shape

## Unsafe Defaults

- Copying a handwritten JWT filter and token service from a tutorial.
- Treating stateless logout as `SecurityContextHolder.clearContext()` only.
- Reusing an access-token parser for refresh tokens without token-type enforcement.
- Disabling CSRF and enabling credentialed CORS without identifying the client model.
- Hardcoding a localhost origin or signing secret.
- Logging every invalid token with a stack trace.
- Adding role tables and authentication endpoints when the accepted phase is unauthenticated.
