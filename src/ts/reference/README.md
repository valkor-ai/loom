# TypeScript Reference Archive

This directory contains the previous TypeScript CLI implementation and legacy agent adapter assets.

It is kept for migration comparison only:

- behavior fixtures
- legacy `.loom/` reader tests
- canonical projection comparison
- manual reference while Rust MCP behavior is reviewed

It is not a product runtime:

- no package from this directory is published as a Loom runtime
- no agent plugin should call this CLI
- Rust MCP tools must not fall back to this implementation
- release packages must not include this directory

The reference package preserves the old repository shape because many reference tests read historical files by path. That is intentional for fixture stability, but it must stay isolated under `src/ts/reference/`.

Reference commands, when explicitly needed for comparison:

```bash
cd src/ts/reference
npm install
npm run build
npm test
```

These commands are not user installation commands. User installation goes through the release installer and `loom-setup`.
