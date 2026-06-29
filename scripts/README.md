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

After a local runtime or plugin fix, refresh the local agent through the Quick Start installer instead of copying binaries or plugin files by hand:

```bash
./install.sh --agent codex --local-build
./scripts/install-local-claude-code.sh
./scripts/install-local-opencode.sh
```

This validates the Rust release build, package layout, `loom-setup install`, MCP registration, and plugin refresh as one path.
