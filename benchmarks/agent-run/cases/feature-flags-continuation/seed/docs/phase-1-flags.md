# Phase 1 Feature Flag Registry Notes

The feature flag registry currently supports a small in-memory workflow: create
flags, enable or disable them, list by enabled state, and summarize basic counts.

Important boundaries:

- Keep the injectable `now()` clock for deterministic tests.
- Keep structured validation errors with `code` and `status`.
- Do not add remote config, percentage hashing, persistence, or dependencies.
- Existing callers rely on `createFlag`, `setFlagEnabled`, `listFlags`, and
  `summarizeFlags`.
