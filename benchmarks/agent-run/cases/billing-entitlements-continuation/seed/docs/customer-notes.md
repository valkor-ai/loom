# Customer Notes

The business team wants a lightweight entitlement model before integrating a
payment provider. They mentioned future ideas such as invoices, trials, coupons,
usage-based billing, and dunning flows.

This phase is narrower: model feature access and usage limits inside the
existing in-memory billing store, then expose over-limit subscriptions and a
summary that helps a human decide the next billing action.
