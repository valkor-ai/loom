# Phase 1 Fulfillment Notes

The fulfillment queue currently supports a small in-memory workflow: add orders,
mark them packed or shipped, list orders by status, and summarize basic queue
counts.

Important boundaries:

- Keep the injectable `now()` clock for deterministic tests.
- Keep structured validation errors with `code` and `status`.
- Do not add carrier labels, warehouse sync, persistence, or package dependencies.
- Existing callers rely on `addOrder`, `markPacked`, `markShipped`, `listOrders`,
  and `summarizeFulfillment`.
