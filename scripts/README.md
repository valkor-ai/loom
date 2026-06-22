# Loom Scripts

Local maintenance helpers for Loom agent plugins. Verification code lives under `tests/`.

## Refresh Local Plugins

During plugin development, Codex installs Loom through the standard personal marketplace flow:

```text
~/.agents/plugins/marketplace.json
~/plugins/loom
```

`npm run plugin:install-codex` refreshes the generated Codex plugin source at `~/plugins/loom`,
updates the personal marketplace entry, adds a Codex cachebuster version to that generated source,
and runs `codex plugin add loom@<marketplace-name>`. Do not hand-write Codex cache directories;
that bypasses Codex's plugin install contract and can leave stale plugin packages active.

Claude Code reads local development plugins from:

```text
$CLAUDE_HOME/skills/loom
```

opencode reads official local command and plugin entries from:

```text
$OPENCODE_CONFIG_HOME/commands
$OPENCODE_CONFIG_HOME/plugins
```

When `OPENCODE_CONFIG_HOME` is not set, Loom refreshes:

```text
~/.config/opencode/commands
~/.config/opencode/plugins
```

Run this before any real Codex, Claude Code, or opencode E2E session:

```bash
npm run plugin:install-adapters
```

The command builds `dist/`, refreshes all adapter packages, writes the shared launcher at `~/.loom/bin/loom-cli`, writes adapter refresh stamps under `~/.loom/adapters/*`, and writes refresh stamps into the installed plugin locations. Agent-facing commands must use this launcher instead of relying on a bare `loom` command in `PATH`. A true E2E run is not valid unless this preflight has completed in the same source revision being tested.

For adapter-specific installs:

```bash
npm run plugin:install-codex
npm run plugin:install-claude
npm run plugin:install-opencode
```

Agent E2E support tools live outside this directory:

```bash
npm run agent:e2e-guard -- /abs/e2e/project-root
npm run audit:claude-log -- --project-root /abs/project --json
```
