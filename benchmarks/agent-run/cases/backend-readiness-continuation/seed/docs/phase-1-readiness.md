# Phase 1 Backend Readiness Notes

The readiness tracker currently supports a small in-memory workflow: add
services, mark them healthy or unhealthy, list by status or owner, and summarize
basic healthy/unhealthy counts.

Important boundaries:

- Keep the injectable `now()` clock for deterministic tests.
- Keep structured validation errors with `code` and `status`.
- Do not add HTTP routes, persistence, Docker, or package dependencies.
- Existing callers rely on `addService`, `markHealthy`, `markUnhealthy`,
  `listServices`, and `summarizeReadiness`.
