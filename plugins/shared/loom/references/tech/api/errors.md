# API Error Handling

Load this file only when `techReferenceProfile.groups.api` includes `errors`.

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
| Unexpected failure | `500` | Generic message plus server-side logging/request id when available. |

## Response Shape

Prefer the repository's existing error envelope. For greenfield HTTP APIs, use a compact problem-details-compatible shape:

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

## Must Not

- Do not return stack traces, SQL errors, ORM exception names, class names, file paths, or dependency internals.
- Do not collapse business blocking into generic `500`.
- Do not make frontend code infer business errors by parsing English prose only.
- Do not return success with an error message body for a failed business operation.

## TaskResult Evidence

For API tasks, `apiContractEvidence` should mention the important error categories covered by code or tests. If a category is intentionally not applicable, record it as a known gap only when the requirement expected it; otherwise keep the evidence focused.
