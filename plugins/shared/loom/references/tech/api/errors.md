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

## Request Tracking And Retry Guidance

When the API touches critical state, background work, or external dependencies, include or preserve a request id so failures can be correlated with logs:

- response header such as `X-Request-ID`
- error body field such as `requestId` or `request_id`
- log correlation in the backend implementation

For retryable failures, declare retry behavior in the accepted API contract or implementation evidence:

- `429` should include `Retry-After` when a meaningful delay exists
- `503` may include `Retry-After` for maintenance or dependency recovery
- business conflicts such as `409` should explain the blocking state instead of asking clients to blindly retry

## Must Not

- Do not return stack traces, SQL errors, ORM exception names, class names, file paths, or dependency internals.
- Do not collapse business blocking into generic `500`.
- Do not make frontend code infer business errors by parsing English prose only.
- Do not return success with an error message body for a failed business operation.
- Do not expose stack traces, SQL messages, class names, filesystem paths, tokens, or dependency internals.

## Implementation Evidence

For API tasks, implementation evidence should mention the important error categories covered by code or tests. If the selected interface includes retry, rate-limit, request-id, or dependency-unavailable behavior, name the covered paths and any known gaps.
