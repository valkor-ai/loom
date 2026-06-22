# Loom Tests

Loom keeps verification code under `tests/` so `scripts/` can stay focused on local maintenance tasks.

Run grouped suites through the shared runner:

```bash
npm run test:deploy
npm run test:knowledge
npm run test:brainstorm
```

Run a single test file by filter:

```bash
node tests/run-suite.js deploy smoke
node tests/run-suite.js knowledge registration
```

Use `tests/harness/` for common repository paths, Loom CLI execution, temporary project roots, and project JSON fixtures. New tests should not copy local `repoRoot`/`cli` runners unless the scenario genuinely needs a different process contract.

`tests/tools/` contains agent/plugin E2E support tools. They are not part of the default test suite because they inspect local agent processes or local agent logs.
