# Spring Boot Security Quality

Use this topic reference when `tech/backend/springboot/security.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`, selected Java security references, and selected API security references when present. This file applies Spring Security 6 rules to task-owned authentication and authorization behavior.

## When To Use

- The task changes `SecurityFilterChain`, authentication endpoints, JWT/resource-server configuration, password handling, method security, CORS/CSRF security behavior, guards for protected endpoints, or current-user resolution.
- Use this when Spring Security ordering, filters, authorities, password encoding, or protected endpoint behavior affects correctness.
- If the current phase explicitly has no authentication/authorization work, do not invent a security system because this reference is available.

## Implementation Focus

- Use modern Spring Security configuration with `SecurityFilterChain`. Do not add deprecated `WebSecurityConfigurerAdapter` patterns.
- Keep authentication concerns separate from business services: token parsing/current-user resolution belongs in security infrastructure; business permission and eligibility rules belong in application/domain code.
- Use `PasswordEncoder` for passwords and never store plaintext or reversible secrets. Keep JWT secrets, issuer/audience, token TTL, and OAuth/resource-server settings in configuration, not source code.
- Apply endpoint authorization rules server-side. UI hiding or controller-only checks are not sufficient for protected operations.
- Prefer method security for service-level permissions when operations can be reached through more than one endpoint or entry point.
- Keep error behavior deliberate: unauthenticated should map to 401, unauthorized to 403, and neither should leak internal resource existence or stack traces.
- Configure CSRF and CORS according to the actual client style: browser session apps, stateless APIs, and same-origin/internal tools have different needs.
- When adding JWT filters or auth converters, make filter order, token absence, invalid token, expired token, and authority mapping explicit.
- Avoid adding rate limiting, refresh-token workflows, or full account management unless the current task owns those behaviors.

## Verification Focus

- Test at least one allowed path, one missing-auth path, and one forbidden/insufficient-role path for protected endpoints.
- Test login/token issuance or token validation paths when the task changes authentication.
- Verify password hashing, invalid credentials, disabled/locked user behavior, and method-security denial when touched.
- Run Spring context startup after security configuration changes so filter chain and bean wiring failures are caught.

## Evidence Notes

- Record `springboot.security` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/backend/springboot/security.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the Spring Security decision: filter chain, auth endpoint, JWT/resource server, method security, password handling, CSRF/CORS, auth error mapping, or protected-path proof.
