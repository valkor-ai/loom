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

Name domain-action subresources for the business result when possible, such as `/orders/{id}/cancellations` or `/requests/{id}/approvals`. Avoid a generic `/actions` endpoint that moves command names into an untyped request body.

## Method Semantics

- `GET` and `HEAD` must not change durable business state. `HEAD` should expose the same metadata headers as the corresponding `GET` without a response body.
- Use `POST` for creation or a non-idempotent domain operation. For retry-sensitive writes, select the operations reference and define the idempotency behavior explicitly.
- Use `PUT` only when the client supplies the complete replacement representation or the repository already defines PUT as an idempotent upsert. Document omitted-field behavior.
- Use `PATCH` for partial changes and declare the patch shape. A normal partial DTO, JSON Merge Patch, and JSON Patch have different null, removal, and validation semantics.
- Define repeated `DELETE` behavior according to the existing contract: stable `204`, later `404`, or another documented idempotent end-state response.
- Use `OPTIONS` and CORS metadata through the framework/runtime convention. Do not implement business behavior in preflight handling.

## Success Status And Headers

| Behavior | Typical Result | Additional Contract |
|---|---|---|
| Immediate read or update | `200` | Response schema and current readback fields. |
| Resource creation | `201` | Created representation or readback; `Location` when a canonical URI exists. |
| Accepted asynchronous work | `202` | Status/result lookup or another concrete completion mechanism. |
| Successful operation with no body | `204` | No response schema or body. |

Do not select a success status by habit. The status, body, headers, and frontend/client expectation must describe the same completion state.

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
- Keep field selection, search, filter, and sort parameters limited to declared fields and operators. Do not let clients pass raw database column names or query fragments.

## Representation And Media Types

- Keep request `Content-Type` and response `Content-Type` aligned with supported serializers.
- Return or document `415` when a task explicitly rejects unsupported request media types. Use `406` only when the API performs real response content negotiation.
- Add multipart, binary, CSV, event-stream, or other media types only for interfaces that own those representations.
- Preserve the repository's JSON field naming convention. Do not rename fields merely to match an unrelated example.

## Verification Hooks

Task verification should prove at least one representative success path and one important blocking/error path for write APIs. Verify the declared status, response body or absence of body, required headers, and readback behavior together. For read APIs, verification should prove the endpoint returns the declared fields and does not silently return fake/static data when the task owns backend implementation.
