# API Contract Artifacts

## Role In Loom

Loom's primary API contract lives in the accepted architecture/API contract. A separate OpenAPI or schema file is optional and should be created only when it has a real consumer.

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

For tasks that touch API contracts, implementation evidence should name generated or updated contract files. If no separate contract file is expected, cite source code and tests instead.

## Runtime And Browser Binding

The accepted API contract is the source of truth for every consumer-facing path. Keep the HTTP interface `path` as the complete public path, including its public prefix; do not make downstream tasks reconstruct it from a generic `/api` label.

Declare the API surface binding once at the contract level:

- `publicExposure.basePath`: the public proxy prefix used by the deployed gateway
- `publicExposure.preservePath`: whether the gateway forwards the interface path unchanged
- `browserBinding.mode`: `same_origin` for a browser served by the Loom public entry, or `external_origin` when an explicit external origin is part of the contract
- `browserBinding.pathOwnership`: `interface_path`

Frontend, integration, browser-verification, and deployment work consumes these accepted values. It must preserve the interface path and request/response contract. It must not add a second base prefix to a path that already contains the public prefix. A separate build-time API environment variable is used only when the frontend source actually composes a relative suffix with that variable; it is not a generic deployment default.
