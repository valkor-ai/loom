# Loom Scripts

This directory is reserved for MCP-only release, packaging, install, and local verification helpers.

Current product runtime entry points are Rust binaries installed from release packages:

- `bin/loom-mcp-server`
- `bin/loom-setup`
- bundled Python algorithm runtime
- Codex, Claude Code, and OpenCode MCP plugin templates

User installation is handled by the release installers. They resolve the host platform, verify the downloaded archive with its `.sha256` release asset, install through `loom-setup`, and run doctor:

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

`npm run rust:test` is the release gate: it compiles workspace libraries and
binary test targets, then compiles the
release integration tests into four domain-grouped binaries before executing
them. The grouped targets keep stateful deployment, MCP, setup, and
verification tests isolated while avoiding one process per source file. The
deployment and workflow groups explicitly run single-threaded because those
tests exercise shared process and filesystem state. Doctests are part of the
full matrix rather than this release gate because the workspace currently has
no doctest cases and Cargo still starts one process per package. The two
independent grouped batches run concurrently; set `LOOM_RUST_TEST_SERIAL=1`
when diagnosing an isolation issue. Use
`npm run rust:test:full` for the complete per-crate unit and integration
matrix, or `npm run rust:test:serial` when diagnosing a process-order or
isolation issue against the original Cargo matrix.

After a local runtime or plugin fix, refresh the local agent through the Quick Start installer instead of copying binaries or plugin files by hand:

```bash
./install.sh --agent codex --local-build
./scripts/install-local-claude-code.sh
./scripts/install-local-opencode.sh
```

This validates the Rust release build, package layout, `loom-setup install`, MCP registration, doctor checks, and plugin refresh as one path.
