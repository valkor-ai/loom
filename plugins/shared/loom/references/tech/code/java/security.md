# Java Security Fundamentals

This reference owns Java-level secure handling of secrets, credentials, cryptographic material, sensitive values, and failure disclosure. Spring Security filter chains, OAuth2 Resource Server, method authorization, CSRF, and CORS belong to the Spring Boot security reference.

## When To Use

Use this reference when Java implementation work handles secrets, credentials, tokens, cryptographic material, sensitive serialization, untrusted deserialization, or security-sensitive failure disclosure. It applies below the web/framework authorization layer.

Do not use it to invent authentication infrastructure or endpoint authorization policy. Framework filter chains, identity providers, method authorization, CSRF, and CORS require an accepted security task and the selected backend security reference.

## Implementation Focus

### Secret Handling

- Never embed production credentials, API keys, signing keys, private keys, or passwords in source.
- Keep secrets out of logs, exception messages, `toString`, metrics tags, traces, and serialized DTOs.
- Prefer typed secret/config providers over scattered environment lookups.
- Avoid retaining sensitive character/byte arrays longer than required; do not copy them unnecessarily.
- Treat test credentials as test-only fixtures, never production defaults.

Configuration files may contain placeholders and non-secret defaults. The unsafe behavior is committing secret values, not using a configuration file format.

### Passwords And Credentials

Use an adaptive password hashing function through the selected security framework. Do not implement password hashing, salting, or comparison manually. Do not store plaintext, reversible passwords, password hints, or password values in audit records.

Credential comparison and token/signature checks must use trusted libraries and constant-time operations where relevant.

### Cryptography

Use standard JCA/JCE or vetted libraries with an accepted algorithm and key-management model. Avoid custom cryptography, insecure random sources, ECB mode, static IVs/nonces, deprecated hashes, or algorithm selection from untrusted input.

Use `SecureRandom` for security-sensitive tokens. Define encoding and key formats explicitly. Key generation, storage, rotation, expiry, and revocation are part of the security contract.

### Sensitive Data Boundaries

Represent sensitive values with types that limit accidental exposure where useful. Keep credential/token fields out of records used for broad JSON serialization. Redact at logging boundaries and avoid logging entire request/response objects.

Validate file paths, URLs, class names, templates, and expression inputs before using APIs that can access the filesystem, network, reflection, deserialization, or code execution.

### Error Disclosure

Return stable safe error categories. Preserve detailed causes server-side only where authorized logs can protect them. Do not reveal whether a sensitive account/resource exists unless the accepted policy permits it.

Avoid catching broad exceptions and returning the exception message. Keep security failures distinct from business validation and unexpected runtime failure.

### Serialization And Deserialization

Use explicit DTO types for untrusted input. Avoid native Java serialization for untrusted data. Configure polymorphic JSON deserialization only with strict allowlists and a concrete need.

## Verification Focus

Useful Java security evidence includes:

- no committed secret values or sensitive logging
- adaptive password encoder use through the selected framework
- secure random/token generation through vetted APIs
- safe DTO/serialization boundaries for sensitive fields
- safe error disclosure
- dependency/static analysis findings for changed security code when available

## Evidence Focus

Evidence must prove the owned security boundary, not merely that the code compiles. Use focused tests for redaction, safe serialization, password/token verification, malformed input, and stable public failures. Include static or dependency analysis only when it evaluates the changed security code and does not replace behavioral proof.

Never place real secret values in test output or evidence. Demonstrate configuration key presence, provider wiring, redaction behavior, and failure classification with synthetic values.

## Unsafe Defaults

- Custom password hashing or token signatures.
- `Random` for security tokens.
- Secrets in constants, source configuration values, or logs.
- Broad object serialization containing credentials.
- Returning raw exception messages to callers.
