# Phase 1 Analytics Notes

The analytics store currently supports a small in-memory workflow: record events,
list them by user or type, and summarize total event volume by type.

Important boundaries:

- Keep deterministic behavior; tests should not depend on wall-clock time.
- Keep structured validation errors with `code` and `status`.
- Do not add external analytics, persistence, batching, or package dependencies.
- Existing callers rely on `recordEvent`, `listEvents`, and `summarizeEvents`.
