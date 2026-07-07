# FastAPI Security Quality

This file applies FastAPI OAuth2, JWT, dependency, and password-handling rules to task-owned authentication and authorization behavior.

## When To Use

- The task changes OAuth2 password flow, JWT creation/validation, password hashing, current-user dependencies, role checks, protected routers, CORS/security middleware, or auth error behavior.
- Use this when endpoint access, token claims, user identity, or permission behavior affects correctness.
- If the current phase explicitly has no auth work, do not add a security layer because this reference is available.

## Implementation Focus

- Keep JWT secrets, algorithms, token lifetimes, issuer/audience, CORS origins, and OAuth2 token URLs in configuration.
- Hash passwords with a maintained password hashing library. Never store plaintext passwords or return password hashes in response models.
- Use `OAuth2PasswordBearer`, current-user dependencies, and role/permission dependencies so protected endpoints cannot bypass auth by direct service access.
- Distinguish authentication failure from authorization failure. Use 401 with `WWW-Authenticate` for invalid/missing credentials and 403 for insufficient permissions.
- Validate token type, subject, expiry, and disabled/deleted-user state. Do not accept refresh tokens where access tokens are required.
- Keep role checks and ownership checks close to the endpoint/service boundary that owns the protected operation.
- Avoid broad global dependencies that accidentally protect public endpoints or leave intended private endpoints open.

## Verification Focus

- Test login/token issuance, invalid credentials, invalid token, expired token, wrong token type, missing auth, insufficient role, and successful protected access when touched.
- Test password hashing and verify plaintext or hashes do not appear in API responses.
- Verify protected routers cannot be reached without the expected dependencies.
- Run endpoint tests with real security dependencies unless the test is explicitly about downstream business logic.

## Evidence Focus

- In the evidence summary, name the FastAPI security decision: OAuth2 flow, JWT claim, password hashing, current-user dependency, role dependency, CORS/security middleware, or protected endpoint proof.
