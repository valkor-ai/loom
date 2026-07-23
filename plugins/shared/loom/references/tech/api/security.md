# API Security And Authorization

## Scope Rule

Declare only the authentication and authorization behavior required by the current phase, existing repository pattern, or confirmed technical baseline. Do not invent OAuth/JWT/session systems for phases that only require local scaffolding or unauthenticated internal prototypes.

## Auth Policy Facts

For protected HTTP APIs, the accepted API contract should state:

- whether authentication is required
- which actors or roles may perform the operation
- which permission or ownership rule applies
- the unauthenticated and unauthorized status behavior
- whether sensitive resource existence should be hidden

Use existing repository roles and permissions when available.

JWT is not the default authentication mechanism. Select it only from an explicitly accepted security profile that matches the current client trust and token-authority scenario. If the profile is absent, do not add JWT or substitute it for a deferred security decision.

For a same-origin browser requirement with an accepted server-session dependency, use the `server_session` profile with `same_origin_cookie`. The backend resolves the authenticated user and roles from the server-side login session, such as the accepted Redis session store, before evaluating `actorRefs` and `permissionRefs`. This is session authentication, not JWT; do not add bearer headers, token claims, issuer, audience, or JWT algorithm settings.

## Implementation Expectations

- Enforce authorization server-side when the task owns backend code.
- UI-only hiding of buttons is not sufficient evidence for protected operations.
- Keep error messages useful but do not leak sensitive resource existence or internal policy details.
- Record auth assumptions as architecture risks when authentication is deferred but the operation is sensitive.

## Verification Hooks

When auth is in task scope, verification should cover at least one allowed path and one denied/missing-auth path. If the current phase intentionally has no auth, do not create fake auth checks; record the decision in Architecture quality or risk when relevant.
