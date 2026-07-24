# API Operational Semantics

Use this reference for API behavior that affects repeated calls, retries, caching, rate limiting, request tracing, or conditional updates. These rules are not mandatory for every endpoint; apply them only when the current phase, repository convention, or accepted API contract needs them. This file owns operational policy; `errors.md` owns error categories, codes, fields, and safe client messages.

## Idempotency

| Operation | Default Expectation | Add Explicit Policy When |
|---|---|---|
| `GET`, `HEAD`, `OPTIONS` | Safe and idempotent. | Cache validators or conditional reads matter. |
| `PUT`, `DELETE` | Idempotent end state. | Concurrent updates or repeated deletes need defined status behavior. |
| `PATCH` | Usually not idempotent unless designed that way. | Partial update can be retried by clients or workers. |
| `POST` | Not idempotent by default. | Payment, submission, workflow transition, import, or duplicate-click scenarios can create duplicate side effects. |

For retry-sensitive `POST` operations, record an idempotency policy in the accepted API contract:

```json
{
  "idempotencyPolicy": {
    "required": true,
    "keyHeader": "Idempotency-Key",
    "dedupeScope": "actor_and_operation",
    "duplicateBehavior": "return_original_result"
  }
}
```

Do not add idempotency storage or headers to simple one-off internal actions unless duplicate submission is a real current-phase risk.

## Caching And Conditional Requests

For read APIs, caching is optional and should follow existing repository or product needs.

Use `cachePolicy` when:

- read payloads are expensive or frequently repeated
- the UI can safely reuse cached data
- the repository already uses `ETag`, `Last-Modified`, or cache-control headers

Use conditional requests when updates need optimistic concurrency:

```json
{
  "cachePolicy": {
    "etag": true,
    "lastModified": false,
    "cacheControl": "private, max-age=60"
  },
  "conditionalRequestPolicy": {
    "ifMatchRequiredForUpdates": true,
    "staleUpdateStatus": 412
  }
}
```

Do not invent cache validators for volatile workflow state unless stale reads are acceptable and described.

## Rate Limiting

Declare a `rateLimitPolicy` only for APIs that are public, login-like, search-heavy, import/batch oriented, or otherwise abuse-sensitive.

```json
{
  "rateLimitPolicy": {
    "applies": true,
    "status": 429,
    "headers": ["Retry-After"],
    "clientMessage": "Too many requests. Try again later."
  }
}
```

For internal tools, rate limiting can be deferred when current deployment or authentication is local-only; record a risk only if abuse or accidental load is plausible in this phase.

## Retry And Availability Responses

Use retry guidance for dependency or runtime outages where retrying can succeed:

- `408`, `429`, `502`, `503`, `504` may be retryable depending on context.
- `400`, `401`, `403`, `404`, `409`, `422` are normally not retryable without user or data changes.
- Include `Retry-After` when the server can give a meaningful delay.

When a dependency is temporarily unavailable, prefer a clear `503` or domain-specific blocking error over generic `500`.

## Request Tracing

For APIs with background work, external dependencies, or important business side effects, define a request id policy:

```json
{
  "requestIdPolicy": {
    "header": "X-Request-ID",
    "includedInErrorBody": true,
    "logCorrelationRequired": true
  }
}
```

An accepted `request_id` operational policy is a structured ownership signal for the application observability reference. It does not require logging every endpoint, and it does not move request tracing or log retention into Deploy.

Implementation evidence should cite tests, runtime probes, logs, or source files proving the selected operational policy. Do not claim operational semantics only in prose.
