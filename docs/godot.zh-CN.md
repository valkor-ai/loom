# Godot 安装

如果希望 Loom 通过 coding agent 操作本地 Godot 项目，可以按下面步骤配置。

## 1. 安装 Godot

根据系统从官方渠道下载 Godot 4.x：

| 系统 | 下载 |
| --- | --- |
| Windows | [下载 Windows 版 Godot](https://godotengine.org/download/windows/) |
| macOS | [下载 macOS 版 Godot](https://godotengine.org/download/macos/) |
| Linux | [下载 Linux 版 Godot](https://godotengine.org/download/linux/) |

确认 Godot 可执行文件的路径。如果 `godot` 不在 `PATH` 中，需要设置
`GODOT_PATH`，供 MCP server 使用。

macOS：

```bash
export GODOT_PATH="/Applications/Godot.app/Contents/MacOS/Godot"
"$GODOT_PATH" --version
```

如果 Godot 放在其他位置，例如：

```bash
export GODOT_PATH="$HOME/Downloads/Godot.app/Contents/MacOS/Godot"
```

Windows PowerShell：

```powershell
$env:GODOT_PATH = "C:\Tools\Godot\Godot_v4.x-stable_win64.exe"
& $env:GODOT_PATH --version
```

Linux：

```bash
chmod +x "$HOME/Applications/Godot_v4.x-stable_linux.x86_64"
export GODOT_PATH="$HOME/Applications/Godot_v4.x-stable_linux.x86_64"
"$GODOT_PATH" --version
```

如果 `godot` 已经在 `PATH` 中，可以不设置 `GODOT_PATH`。

## 2. 安装 Loom

根据使用的 agent 安装 Loom：

```bash
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent codex
```

需要时将 `codex` 替换为 `claude-code`、`opencode` 或 `all`。安装后重新
打开 agent 会话。

## 3. 注册 Godot MCP

`godot-mcp` 是独立于 Loom 的 MCP server，需要注册为 `godot`。

Codex：

```bash
codex mcp add godot --env GODOT_PATH="$GODOT_PATH" -- npx @coding-solo/godot-mcp
```

Claude Code：

```bash
claude mcp add godot -e GODOT_PATH="$GODOT_PATH" -- npx @coding-solo/godot-mcp
```

OpenCode 配置：

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

将示例路径替换为本机 Godot 可执行文件的实际路径。

## 4. 验证

创建或打开一个根目录包含 `project.godot` 的 Godot 项目，然后重新打开 agent
会话。通过 `godot` MCP server 调用 `get_godot_version` 或 `get_project_info`。
Loom 是独立的 MCP server，需要和 Godot MCP 同时保留。
