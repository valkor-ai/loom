# Testing Discipline

Use this reference when adding or changing tests, choosing a verification seam, or recording verification evidence.

## Choose the seam

- Test observable behavior through the highest stable interface available.
- Prefer one narrow end-to-end slice that proves the delivery path over separate layer-only checks.
- Avoid tests coupled to private implementation unless the task explicitly asks for that seam.
- Reuse existing test patterns and project commands before introducing new harnesses.

## Write useful tests

- Name tests in the product/domain language used by the request.
- Assert the behavior that matters to the user or contract, not incidental structure.
- Keep each test focused enough that a failure points to a real obligation.
- Do not add broad snapshots or weak "does not throw" checks as primary evidence.

## Red-green when possible

- For bug fixes, make the bug red before fixing when a stable seam exists.
- For features, use a thin tracer path: one behavior, one implementation step, one verification loop.
- Do not write a batch of imagined tests far ahead of the implementation.

## Record evidence

- Run the smallest meaningful verification command first, then broader checks when needed.
- Report exact commands and outcomes in the Loom result artifact.
- If a test cannot be added, record the reason: no stable seam, missing environment, external dependency, or out-of-scope risk.
