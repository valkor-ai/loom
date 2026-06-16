# Phase 1 Billing Store Notes

The billing store currently supports a small in-memory workflow: create plans,
assign customer subscriptions, list plans or subscriptions, and summarize basic
plan/subscription counts.

Important boundaries:

- Keep the injectable `now()` clock for deterministic tests.
- Keep structured validation errors with `code` and `status`.
- Do not add payment provider calls, invoices, persistence, or dependencies.
- Existing callers rely on `createPlan`, `assignSubscription`, `listPlans`,
  `listSubscriptions`, and `summarizeBilling`.
