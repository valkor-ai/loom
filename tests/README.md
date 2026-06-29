# Loom Tests

Product verification is split by runtime:

```bash
npm run rust:test
npm run python:test
```

Rust tests cover the MCP server, state protocol, request read contracts, setup/install behavior, knowledge, planning, execution, review, repair, and deploy runtime behavior. Python tests cover algorithm-worker behavior.
