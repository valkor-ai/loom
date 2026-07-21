# API Error Handling

## Error Contract Purpose

API errors are part of product behavior. They should let the caller or UI explain what happened and what can be done next without exposing implementation internals.

## Required Error Categories

| Category | Status Guidance | Contract Requirement |
|---|---|---|
| Validation | `400` or `422` according to project convention | Field or request-level reason. |
| Business conflict/blocking | `409` or domain-specific `4xx` | Stable code and user-actionable message. |
| Not found | `404` | Resource type/id context when safe. |
| Auth missing/invalid | `401` | Authentication required or expired. |
| Permission denied | `403` | Action not allowed, no sensitive detail leakage. |
| Rate limit | `429` | Retry timing and stable client behavior when throttled. |
| Temporary unavailable | `503` | Dependency or maintenance outage that may succeed later. |
| Unexpected failure | `500` | Generic message plus server-side logging/request id when available. |

## Response Shape

Prefer the repository's existing error envelope. For new-project HTTP APIs, use a compact problem-details-compatible shape:

```json
{
  "type": "business_conflict",
  "title": "Request cannot be completed",
  "status": 409,
  "detail": "The purchase request cannot be approved before budget review.",
  "code": "BUDGET_REVIEW_REQUIRED",
  "fields": []
}
```

The `type` can be a stable string or URI according to project convention. Do not require full RFC 7807 URI infrastructure when the product is an internal app and no API documentation surface exists yet.

## Validation Details

For write APIs, error contracts should distinguish field-level and cross-field validation when those rules exist.

```json
{
  "type": "validation",
  "status": 422,
  "code": "VALIDATION_ERROR",
  "fields": [
    { "field": "amount", "code": "OUT_OF_RANGE", "message": "Amount must be greater than zero." },
    { "fields": ["startDate", "endDate"], "code": "INVALID_RANGE", "message": "End date must be after start date." }
  ]
}
```

Keep stable machine-readable `code` values even when visible messages are localized or translated by the client.

## Error Code Ownership

- Define stable error codes for validation, business blocking, conflicts, not-found behavior, and auth failures that clients handle programmatically.
- Keep one code catalog or source of truth when multiple endpoints share the same error. Do not create different codes for the same business condition in controller, documentation, and frontend code.
- Document the error categories and codes applicable to each interface when a separate contract file is selected. Do not claim every global error on every endpoint.
- Keep user-visible messages actionable and safe. Clients should branch on stable codes, not parse translated prose.

## Operational Policy Link

This reference owns error categories, codes, fields, and safe messages. When `tech/api/operations.md` is selected, use its request-id, retry, rate-limit, and availability policy without restating or inventing a second operational contract here.
Application-side correlation, redaction, log levels, and diagnostic retention belong to `tech/code/observability.md` when the task owns that boundary.

## Must Not

- Do not return stack traces, SQL errors, ORM exception names, class names, file paths, or dependency internals.
- Do not collapse business blocking into generic `500`.
- Do not make frontend code infer business errors by parsing English prose only.
- Do not return success with an error message body for a failed business operation.

## Implementation Evidence

For API tasks, implementation evidence should mention the important error categories and stable codes covered by code or tests. When an operations policy is selected, cite its evidence separately rather than duplicating it in the error summary.
