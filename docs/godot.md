# Godot Setup

Use this setup when you want Loom to work with a local Godot project through a
coding agent.

## 1. Install Godot

Install Godot 4.x from the official download page for your platform:

| Platform | Download |
| --- | --- |
| Windows | [Godot for Windows](https://godotengine.org/download/windows/) |
| macOS | [Godot for macOS](https://godotengine.org/download/macos/) |
| Linux | [Godot for Linux](https://godotengine.org/download/linux/) |

Keep the executable path available to the MCP server. Set `GODOT_PATH` when
Godot is not already on `PATH`.

macOS:

```bash
export GODOT_PATH="/Applications/Godot.app/Contents/MacOS/Godot"
"$GODOT_PATH" --version
```

The app can live somewhere else, for example:

```bash
export GODOT_PATH="$HOME/Downloads/Godot.app/Contents/MacOS/Godot"
```

Windows PowerShell:

```powershell
$env:GODOT_PATH = "C:\Tools\Godot\Godot_v4.x-stable_win64.exe"
& $env:GODOT_PATH --version
```

Linux:

```bash
chmod +x "$HOME/Applications/Godot_v4.x-stable_linux.x86_64"
export GODOT_PATH="$HOME/Applications/Godot_v4.x-stable_linux.x86_64"
"$GODOT_PATH" --version
```

If `godot` is already on `PATH`, `GODOT_PATH` is optional.

## 2. Install Loom

Install Loom for the agent you use:

```bash
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent codex
```

Replace `codex` with `claude-code`, `opencode`, or `all` when needed. Open a
new agent session after installation.

## 3. Register Godot MCP

`godot-mcp` is a separate MCP server from Loom. Register it as `godot`.

Codex:

```bash
codex mcp add godot --env GODOT_PATH="$GODOT_PATH" -- npx @coding-solo/godot-mcp
```

Claude Code:

```bash
claude mcp add godot -e GODOT_PATH="$GODOT_PATH" -- npx @coding-solo/godot-mcp
```

OpenCode configuration:

```json
{
  "mcp": {
    "godot": {
      "type": "local",
      "command": ["npx", "@coding-solo/godot-mcp"],
      "environment": {
        "GODOT_PATH": "/path/to/Godot"
      }
    }
  }
}
```

Replace the path with the actual Godot executable on your machine.

## 4. Verify

Create or open a Godot project whose root contains `project.godot`, then start
a new agent session. Call `get_godot_version` or `get_project_info` through the
`godot` MCP server. Loom remains a separate MCP server and should stay
installed alongside it.
