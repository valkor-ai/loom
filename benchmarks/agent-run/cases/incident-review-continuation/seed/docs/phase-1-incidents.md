# Phase 1 Incident Tracker Notes

The incident tracker currently supports a small in-memory workflow: add
incidents, assign owners, resolve incidents, list by status or severity, and
summarize open/resolved counts.

Important boundaries:

- Keep the injectable `now()` clock for deterministic tests.
- Keep structured validation errors with `code` and `status`.
- Do not add alert-provider integrations, paging, persistence, or dependencies.
- Existing callers rely on `addIncident`, `assignIncident`, `resolveIncident`,
  `listIncidents`, and `summarizeIncidents`.
