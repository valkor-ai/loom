# API Resource Design

Load this file only when the current MCP request lists `tech/api/resource.md` in `referenceLoadPlan`.

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

## AAC Interface Shape

For HTTP APIs, Architecture `interfaces[]` should include the fields that are known in the current phase:

```json
{
  "interfaceId": "api_purchase_request_create",
  "name": "Create purchase request",
  "type": "http_api",
  "resource": "purchase_requests",
  "operationKind": "create",
  "method": "POST",
  "path": "/api/purchase-requests",
  "requestSchema": [{ "field": "title", "required": true, "kind": "string" }],
  "responseSchema": [{ "field": "id", "required": true, "kind": "identifier" }],
  "statusCodes": {
    "success": [201],
    "validation": [400, 422],
    "businessConflict": [409],
    "notFound": [],
    "auth": [401, 403]
  },
  "errorSchema": [{ "field": "message", "required": true, "kind": "user_actionable_message" }],
  "scopeRefs": [],
  "acceptanceRefs": []
}
```

Use project conventions for exact JSON field naming. The Loom contract should preserve the semantic fields even when the implementation uses framework-specific DTO names.

## Path Rules

- Use plural, stable resource names for collections.
- Keep nesting shallow; prefer `/orders/{orderId}/items` over deeply nested chains.
- Use query parameters for filtering, sorting, and pagination.
- Do not expose table names, ORM names, package names, or implementation class names as API resources.
- Use the existing route prefix when the repository already has one.

## Verification Hooks

Task verification should prove at least one representative success path and one important blocking/error path for write APIs. For read APIs, verification should prove the endpoint returns the declared fields and does not silently return fake/static data when the task owns backend implementation.
