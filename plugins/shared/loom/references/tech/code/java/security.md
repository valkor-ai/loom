# Java Spring Security Quality

Use this topic reference when `tech/code/java/security.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. This file applies only to task-owned authentication, authorization, credential, CORS/CSRF, or security error behavior.

## When To Use

- The task changes Spring Security configuration, login/auth endpoints, JWT/session handling, role/permission checks, password storage, method security, CORS/CSRF, or protected API behavior.
- Do not introduce Spring Security, JWT, OAuth2, or role models for a phase that does not require protected operations.
- If the repository already has an auth mechanism, extend that mechanism instead of adding a second parallel security path.

## Implementation Focus

- Define public and protected routes explicitly in `SecurityFilterChain`. Do not leave broad `permitAll` or broad `authenticated` rules when the task's accepted scope names concrete actors or operations.
- Keep filter order deliberate. JWT/custom filters belong before the appropriate Spring Security authentication filter; do not add filters without proving they run for the intended routes.
- Externalize secrets, token durations, issuers, audiences, allowed origins, and password policy. Never hardcode JWT secrets, generated keys, or dev-only origins in application code.
- Use a password encoder such as BCrypt/Argon2 for stored credentials. Do not store plaintext, reversible passwords, or test passwords in source.
- Model authorities consistently. If the project uses `ROLE_ADMIN`, method annotations and route matchers must use the same role/authority convention.
- Prefer method security for business operation authorization when the decision depends on domain ownership or resource state; route-level checks are not enough for object-level access.
- CORS and CSRF depend on client type. Browser session apps usually need CSRF; stateless bearer-token APIs often disable CSRF deliberately. Record the reason in code or evidence when changing defaults.
- Map auth failures and access denials into the project's error response contract. Do not expose token parsing exceptions, stack traces, or account internals to clients.
- Security changes must not break actuator health or explicitly public endpoints unless the task says they should be protected.
- Test users, tokens, and roles should live in test fixtures/config, not production defaults.

## Verification Focus

- Verify at least one allowed request and one denied request for every changed protected route or method rule.
- For JWT/session changes, test missing token, malformed/expired token when feasible, valid token, and insufficient authority.
- For password/login changes, test invalid credentials and successful authentication without exposing secrets in logs or responses.
- For CORS/CSRF changes, run a targeted test or probe that matches the actual frontend/client behavior.
- Run the Spring test/build command after security changes because filter chain wiring often fails at context startup.

## Evidence Notes

- Record `java.security` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/java/security.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, state the protected routes/methods, allowed/denied cases verified, and how credentials/secrets are externalized.
