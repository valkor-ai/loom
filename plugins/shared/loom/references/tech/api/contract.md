# API Contract Artifacts

## Role In Loom

Loom's primary API contract lives in the project-level current API contract referenced by the accepted Architecture artifact. The Architecture artifact records `apiContractRef` plus current-phase interface refs; downstream requests receive only the projection needed by their task. A separate OpenAPI or schema file is optional and should be created only when it has a real consumer.

## When To Create Or Update OpenAPI

Create or update an OpenAPI file when at least one condition is true:

- the user explicitly asks for OpenAPI/API documentation
- the repository already maintains an OpenAPI file
- client/server code generation depends on it
- the API is external/public or consumed by another separately versioned application
- Review or deploy needs a stable contract file to validate endpoints

Do not create OpenAPI only to satisfy a generic reference rule.

## Contract File Expectations

When OpenAPI is required:

- Prefer OpenAPI 3.1 unless the repository already uses another version.
- Keep operation ids stable and resource-oriented.
- Group operations with tags that match business resources or modules.
- Reuse component schemas for request bodies, responses, errors, pagination, and auth when that reduces drift.
- Include success and error response schemas for the status categories declared in the accepted API contract.
- Include examples only when they clarify business behavior, important validation, or state transitions.
- Validate with the project's existing command if available; otherwise record static evidence and known gaps rather than adding heavy tooling.

## Operation Objects

- Give every operation a stable, unique `operationId` when the repository, documentation, or code generation consumes it. Preserve existing ids during compatible changes.
- Define every path parameter in the operation or path item, mark it required, and keep its name identical to the path template.
- Describe query parameters with their accepted type, bounds, enum, array serialization, and default only when the implementation actually provides that behavior.
- Use `requestBody.required` according to the accepted interface. Keep media types aligned with the implementation instead of advertising JSON, multipart, form, or binary payloads that are not supported.
- Define every accepted success and important error response with the correct status, headers, media type, and schema. A description without the response shape is insufficient when clients consume the body.
- Represent `201` creation responses with the created resource or accepted readback shape and a `Location` header when the implementation exposes the canonical resource URI.
- Represent `202` only when processing continues asynchronously; document the operation/status resource, result lookup, or callback mechanism that clients can actually use.
- A `204` response has no response body. Do not attach a JSON schema or example to it.

## OpenAPI 3.1 Schema Semantics

- Treat OpenAPI 3.1 schemas as JSON Schema 2020-12. Model nullable values with a null union such as `type: [string, "null"]` or an equivalent `oneOf`; do not use the OpenAPI 3.0 `nullable` keyword in a 3.1 document.
- Put property names in the enclosing object's `required` array. Do not confuse a required request body with required object properties.
- Use `readOnly` for server-produced fields and `writeOnly` for accepted secrets or write-only inputs. Keep request and response schemas separate when their required fields or lifecycle differ materially.
- Apply `minLength`, `maxLength`, `pattern`, numeric bounds, `minItems`, `maxItems`, and `uniqueItems` only when the accepted validation and implementation enforce them.
- Treat `format` as an interoperability hint unless the selected validator enforces it. Do not claim email, UUID, URI, date, or date-time validation that the implementation does not perform.
- Choose `additionalProperties` deliberately. Closed DTOs may reject unknown fields; extension maps need a typed additional-property schema.
- Use `oneOf` for exclusive alternatives and add a discriminator when clients need deterministic subtype selection. Use `allOf` for genuine schema composition, not as a substitute for unclear inheritance.
- Keep enum values, defaults, examples, nullability, and constraints aligned with source DTOs, persistence semantics, and accepted API behavior.

## Components, Security, And Examples

- Reuse component schemas, parameters, responses, headers, and security schemes when reuse reduces contract drift. Do not create a component for a value used once when the indirection makes the operation harder to inspect.
- Declare bearer, API-key, OAuth, cookie, or mutual-TLS schemes only when the accepted auth policy selects them. Keep scopes and operation-level requirements aligned with actual authorization checks.
- Use operation-level security overrides only for intentionally public or differently protected operations; do not accidentally erase a global requirement with an empty security array.
- Examples must satisfy their schemas, avoid real credentials or personal data, and demonstrate meaningful success, validation, or business-conflict behavior.
- Keep tags aligned with business resources or modules. Do not use framework package names or database tables as API documentation groups.

## Validation And Generation

- Prefer the repository's existing OpenAPI validation command and version. Do not install Redocly, Spectral, Swagger CLI, Prism, or a generator solely because this reference mentions validation.
- When no validator exists, perform a structured static check for resolvable `$ref` values, unique `operationId` values, path-parameter agreement, declared response schemas, and example/schema consistency.
- Generate clients or server stubs only when the repository already owns code generation or the current task names a consumer. Record the generator, version, input file, output ownership, and regeneration command.
- Do not hand-edit generated clients or server stubs when the source contract and generator own those files.

## Minimal Professional Spec Contents

When a separate contract file is selected, the agent should preserve or create:

- `openapi`, `info`, and `servers` only when server URLs are known or already established
- `paths` with method, path params, query params, request body, and response shapes
- `operationId` values that are stable and action/resource oriented
- `components.schemas` for reusable DTOs and error envelopes
- `components.responses` for shared errors when the repo convention supports them
- security schemes only when the current phase or existing repo actually requires auth

Do not add a generated OpenAPI file that disagrees with source code, the accepted API contract, or tests just to satisfy documentation style.

## Existing Contract Files

If the repository already owns an OpenAPI/schema file:

- update it in the existing style and version
- avoid reformatting unrelated paths
- preserve existing servers/security conventions unless the task changes them
- cite the file in implementation evidence

## Implementation Evidence

For tasks that touch API contracts, implementation evidence should name generated or updated contract files, validation commands or static checks, and any generated consumer affected. If no separate contract file is expected, cite source code and tests instead.

## Runtime And Browser Binding

The accepted API contract is the source of truth for every consumer-facing path. Keep the HTTP interface `path` as the complete public path, including its public prefix; do not make downstream tasks reconstruct it from a generic `/api` label.

Declare the API surface binding once at the contract level:

- `publicExposure.basePath`: the public proxy prefix used by the deployed gateway
- `publicExposure.preservePath`: whether the gateway forwards the interface path unchanged
- `browserBinding.mode`: `same_origin` for a browser served by the Loom public entry, or `external_origin` when an explicit external origin is part of the contract
- `browserBinding.pathOwnership`: `interface_path`

Frontend, integration, browser-verification, and deployment work consumes these accepted values. It must preserve the interface path and request/response contract. It must not add a second base prefix to a path that already contains the public prefix. A separate build-time API environment variable is used only when the frontend source actually composes a relative suffix with that variable; it is not a generic deployment default.
