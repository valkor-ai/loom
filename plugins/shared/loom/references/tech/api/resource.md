# API Resource Design

## Resource Modeling

Model APIs around business resources and state transitions.

| Operation Kind | Typical Method | Path Shape | Required Contract |
|---|---|---|---|
| Create | `POST` | `/resources` | request body, `201` or accepted status, created response/readback |
| Read list | `GET` | `/resources` | filters, pagination policy when needed, stable response collection |
| Read detail | `GET` | `/resources/{id}` | path id, not-found behavior, detail response |
| Replace | `PUT` | `/resources/{id}` | full replacement fields and idempotency expectation |
| Update | `PATCH` | `/resources/{id}` | partial fields and validation |
| Delete/close/cancel | `DELETE` or `POST` subresource | `/resources/{id}` or `/resources/{id}/cancellations` | state transition, conflict/blocking errors |
| Domain action | `POST` subresource | `/resources/{id}/actions` | use only when the domain operation is not a simple CRUD state mutation |

## Interface Shape

For HTTP APIs, the accepted API contract should preserve the semantic fields known in the current phase:

- stable endpoint id and human-readable operation name
- resource name and operation kind
- HTTP method and path
- request body, query, and path fields
- success response fields and readback fields
- success, validation, business-blocking, not-found, and auth status behavior
- stable error body fields such as code and user-actionable message
- requirement or acceptance links when the delivery contract provides them

Use project conventions for exact JSON field naming. The contract should preserve the semantic fields even when the implementation uses framework-specific DTO names.

## Path Rules

- Use plural, stable resource names for collections.
- Keep nesting shallow; prefer `/orders/{orderId}/items` over deeply nested chains.
- Use query parameters for filtering, sorting, and pagination.
- Do not expose table names, ORM names, package names, or implementation class names as API resources.
- Use the existing route prefix when the repository already has one.

## Verification Hooks

Task verification should prove at least one representative success path and one important blocking/error path for write APIs. For read APIs, verification should prove the endpoint returns the declared fields and does not silently return fake/static data when the task owns backend implementation.
