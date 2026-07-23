# JWT API Contract

## Scope

JWT is a retained but dormant capability. Use this reference only when an explicitly accepted TechnicalBaseline security profile selects `mechanism=bearer_jwt` for the current security scenario. A protected requirement, backend framework, keyword, or greenfield project does not activate JWT. This file defines the API-facing obligations of an active profile; it does not select an algorithm, create login endpoints, or replace the framework security reference.

When no active profile exists, keep the current phase free of JWT implementation and record the security decision as pending or deferred through the owning contract. Do not invent a JWT profile to make an unauthenticated implementation pass.

## Contract Boundary

- Read the selected `securityProfileRef` from the accepted interface `authPolicy`.
- Accept only the algorithm selected by that profile. Never derive or widen the algorithm from an incoming token header.
- Keep issuer, audience, key source, token type, clock skew, and claim names aligned with the profile and external identity contract.
- Confirm that the scenario has an identified token authority, client trust model, and authorization mapping before wiring a verifier. The current JWT capability does not by itself define login, refresh, revocation, tenant, or ownership semantics.
- Validate signature, issuer, audience, expiry, not-before, subject, and token type when those claims are part of the profile.
- Keep access-token and refresh-token contracts separate. Do not add refresh tokens, login, logout, revocation, or user storage unless the accepted API contract owns them.

## HTTP Behavior

Protected interfaces must define stable `401` and `403` behavior, including the accepted safe error envelope. Missing or invalid bearer credentials are not business validation errors. Do not reveal account existence, parser details, signing keys, raw claims, or token validation internals.

## Key And Configuration Boundary

The profile owns the key source, not the secret value. Bind issuer, audience, JWK location or secret reference, selected algorithm, token lifetime, and clock skew through validated configuration. Never commit signing material or log authorization headers, access tokens, refresh tokens, or decoded sensitive claims.

## Verification Evidence

For a task that owns JWT behavior, provide focused evidence for allowed access, missing credentials, malformed or invalid signatures, expired tokens, wrong issuer or audience, wrong token type, and insufficient permission as applicable to the selected contract. A token extractor, OpenAPI security scheme, or library import alone is not evidence of validation.

## Non-Goals

Do not add JWT to an unauthenticated interface, choose multiple algorithms “for flexibility,” write a handwritten parser when the selected framework has a maintained resource-server path, or copy this guidance into language/framework references.
