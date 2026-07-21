# Redis Session Integration

## When To Use

Use this reference only when the task owns the accepted Redis `session` capability or changes login-state, session lookup, session renewal, logout, or session invalidation behavior.

## Implementation Focus

- Define whether Redis stores the authoritative session state or only a shared lookup for an application-owned session record.
- Use a dedicated namespace with a non-guessable session identifier; never use a user id as the session key.
- Store only the minimum identity, expiry, authorization-version, and revocation data needed by the session boundary.
- Set an explicit idle or absolute expiration on every session and define renewal behavior before expiry.
- Invalidate sessions on logout, credential change, permission revocation, and security-sensitive account changes when the accepted policy requires it.
- Make session serialization versioned and reject incompatible payloads instead of silently accepting partial state.
- Keep cookie or token attributes, rotation, CSRF protection, and transport security in the application authentication boundary; Redis is only the shared state store.
- For multi-instance services, use the Compose or runtime service name rather than `localhost` and bound connection timeout and pool behavior.

## Failure Boundary

Define the behavior when Redis is unavailable or a session is missing. A required session store must fail closed with an actionable authentication response; a declared fallback must not turn an expired, revoked, or malformed session into an authenticated request.

## Scope Boundary

- Keep password hashing, token signing, cookie construction, CSRF checks, and authorization policy in the application authentication module.
- Keep Redis-specific key, expiry, serialization, and unavailable-provider behavior in one session adapter.
- Do not make a session reference the reason to add Redis when the accepted baseline does not select the session capability.

## Verification Focus

- Two application instances can read the same valid session without leaking another user or tenant's state.
- Idle and absolute expiration, renewal, logout, revocation, and credential-change invalidation follow the accepted policy.
- Session identifiers cannot be enumerated or derived from user identity.
- Redis unavailable, timeout, malformed payload, and stale-version behavior fail safely.
- Cookie or token rotation and authorization-version changes do not leave an old session usable.

## Evidence Focus

Record the session namespace, identity fields, expiration policy, renewal and invalidation triggers, runtime URL, unavailable-provider behavior, and focused authentication tests.

## Anti-Patterns

- Using `user:{id}` as a bearer session key.
- Treating Redis availability as proof that a session is authenticated.
- Omitting expiration or extending a session on every request without an absolute limit.
- Storing passwords, raw credentials, or unrestricted domain objects in session values.
- Falling back to an old session after a logout, revocation, or payload-version failure.
