# Phase 1 Customer Onboarding Notes

The onboarding tracker currently supports a compact in-memory workflow: add
customers, move them between stages, list customers by stage or owner, and
summarize stage counts.

Important boundaries:

- Keep the injectable `now()` clock for deterministic tests.
- Keep structured validation errors with `code` and `status`.
- Do not add CRM sync, persistence, reminders, or package dependencies.
- Existing callers rely on `addCustomer`, `setStage`, `listCustomers`, and
  `summarizeOnboarding`.
