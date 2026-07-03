# API Evolution And Compatibility

Load this file only when `techReferenceProfile.groups.api` includes `evolution`.

## Default Position

Loom does not require API versioning by default. Do not add `/v1`, deprecation headers, migration guides, or version routers unless the current contract actually needs API lifecycle management.

## When Evolution Rules Apply

Apply compatibility rules when one or more are true:

- the user explicitly asks for versioning or public API stability
- existing repository APIs already use versions
- current work changes an existing interface consumed by another app
- OpenAPI/SDK/codegen clients depend on stable schema
- the task modifies response fields, status codes, authentication, or request requirements of an accepted interface

## Compatibility Rules

Usually compatible:

- adding optional request fields
- adding response fields that clients can ignore
- adding new endpoints
- tightening server-side validation only when invalid data was already rejected by business rules

Usually breaking:

- removing or renaming fields
- changing field types
- adding required request fields
- changing success/error status codes for the same scenario
- changing auth behavior
- changing response envelope shape

## Loom Handling

When a breaking change risk exists:

- Architecture should record a risk or decision.
- TaskPlan should assign it to the task touching that interface.
- Execution should either preserve compatibility or record the intentional change.
- Review should route unacknowledged compatibility breaks to architecture or execution repair depending on where the gap lives.

## Deprecation And Sunset

When the current phase intentionally deprecates an accepted API, define the policy rather than only changing code:

- successor endpoint or response shape
- deprecation signal, such as `Deprecation` response header or documentation note
- sunset timing when known
- migration note or compatibility adapter when a separate client exists

Use `Sunset` and `Link: rel="successor-version"` headers only when the product actually has clients that can consume them. Internal apps can record a simpler migration note.

## Version Discovery

Do not create root version discovery endpoints by default. Add discovery only when:

- the existing API already exposes version metadata
- public or separately deployed clients need to negotiate versions
- OpenAPI/SDK distribution depends on separate versions

## Contract File Handling

If `contract` is also selected, keep versioning reflected in the contract file:

- separate specs per major version when existing repository convention uses that
- otherwise a single spec with clear server/path conventions
- no hidden breaking changes without an architecture risk or decision
