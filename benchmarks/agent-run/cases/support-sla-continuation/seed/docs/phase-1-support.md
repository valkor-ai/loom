# Phase 1 Support Queue Notes

The support queue currently supports a small in-memory workflow: add tickets,
assign tickets, resolve tickets, list by status or assignee, and summarize basic
open/resolved progress.

Important boundaries:

- Keep the injectable `now()` clock for deterministic tests.
- Keep structured validation errors with `code` and `status`.
- Do not add persistence, HTTP routing, background workers, or dependencies.
- Existing callers rely on `addTicket`, `assignTicket`, `resolveTicket`,
  `listTickets`, and `summarizeQueue`.
