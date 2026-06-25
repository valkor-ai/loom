# Loom Tests

Product verification is split by runtime:

```bash
npm run rust:test
npm run python:test
```

Rust tests cover the MCP server, state protocol, request read contracts, setup/install behavior, knowledge, planning, execution, review, repair, and deploy runtime behavior. Python tests cover algorithm-worker behavior.

The previous TypeScript CLI test lane is archived under `tests/ts/reference/` and runs only as a migration comparison suite:

```bash
npm run reference:typescript
```

Reference tests are not product runtime tests and must not be used as a fallback for MCP behavior.
