# Phase 1 Compliance Evidence Notes

The evidence store currently supports a small in-memory workflow: add evidence,
change review status, list evidence by owner or status, and summarize accepted
versus pending items.

Important boundaries:

- Keep the injectable `now()` clock for deterministic tests.
- Keep structured validation errors with `code` and `status`.
- Do not add file uploads, persistence, auditor exports, or package dependencies.
- Existing callers rely on `addEvidence`, `setEvidenceStatus`, `listEvidence`,
  and `summarizeEvidence`.
