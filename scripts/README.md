# Loom Scripts

This directory is reserved for MCP-only release, packaging, install, and local verification helpers.

Current product runtime entry points are Rust binaries installed from release packages:

- `bin/loom-mcp-server`
- `bin/loom-setup`
- bundled Python algorithm runtime
- Codex, Claude Code, and OpenCode MCP plugin templates

User installation is handled by the release installers:

```bash
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent codex
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent claude-code
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent opencode
```

Developer verification should use the product test lanes:

```bash
npm run rust:test
npm run python:test
```

TypeScript CLI and legacy adapter refresh scripts are archived under `src/ts/reference/` for migration comparison only. They are not product install paths, not release packaging inputs, and not fallback runtimes.
