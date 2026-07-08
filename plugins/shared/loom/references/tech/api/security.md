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

## Implementation Expectations

- Enforce authorization server-side when the task owns backend code.
- UI-only hiding of buttons is not sufficient evidence for protected operations.
- Keep error messages useful but do not leak sensitive resource existence or internal policy details.
- Record auth assumptions as architecture risks when authentication is deferred but the operation is sensitive.

## Verification Hooks

When auth is in task scope, verification should cover at least one allowed path and one denied/missing-auth path. If the current phase intentionally has no auth, do not create fake auth checks; record the decision in Architecture quality or risk when relevant.
