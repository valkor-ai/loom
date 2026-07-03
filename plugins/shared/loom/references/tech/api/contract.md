# API Contract Artifacts

Load this file only when `techReferenceProfile.groups.api` includes `contract`.

## Role In Loom

Loom's primary API contract lives in AAC `interfaces[]`. A separate OpenAPI or schema file is optional and should be created only when it has a real consumer.

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
- Include success and error response schemas.
- Include examples only when they clarify business behavior.
- Validate with the project's existing command if available; otherwise record static evidence and known gaps rather than adding heavy tooling.

## TaskResult Evidence

For tasks that touch API contracts, `apiContractEvidence.contractFileRefs` should list generated or updated contract files. If no separate contract file is expected, leave the list empty and cite source code/tests instead.
