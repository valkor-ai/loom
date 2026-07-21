# godot-mcp Tool Reference

Complete reference for all tools provided by the `@coding-solo/godot-mcp` MCP server.

## Table of contents

- [Debug tools](#debug-tools)
- [Project info tools](#project-info-tools)
- [Scene management tools](#scene-management-tools)
- [UID tools](#uid-tools)

---

## Debug tools

These are the primary tools for runtime debugging — the core of the mcp-driver workflow.

### run_project

Launch a Godot project in debug mode and begin capturing output.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `projectPath` | string | yes | Path to directory containing `project.godot` |
| `scene` | string | no | Specific scene to run (e.g. `res://scenes/main.tscn`). Omit to run the project's default main scene. |

**Returns:** Confirmation message that the project was launched. Output is NOT returned here — call `get_debug_output` separately to retrieve it.

**Behavior:**
- Kills any previously running Godot process before starting a new one
- The project runs as a child process; output accumulates until you call `get_debug_output`
- The project keeps running until you call `stop_project` or it exits on its own

**Gotchas:**
- `projectPath` must be an absolute OS path (e.g. `D:/MyGame` or `/home/user/mygame`), not a `res://` path
- If the project has no main scene set and you don't specify `scene`, Godot will show the project manager instead of running the game
- On Windows, use forward slashes in the path — godot-mcp handles conversion

**When to use:** Start here when you need to see what actually happens at runtime. Prefer `headless-build` for compile-only checks.

---

### get_debug_output

Retrieve accumulated console output (stdout and stderr) from the running project.

**Parameters:** None.

**Returns:** JSON object with two arrays:

```json
{
  "output": ["line 1", "line 2", ...],
  "errors": ["error line 1", ...]
}
```

- `output` — everything printed to stdout (print statements, engine info, warnings)
- `errors` — everything printed to stderr (script errors, engine errors)

**Gotchas:**
- Returns empty arrays if no output has been produced yet — the project may still be starting up
- Output accumulates between calls; each call returns all output since the project started (not just new output)
- Call this after a short delay (~2-3 seconds) to let the project initialize and produce output
- Long-running projects will accumulate large output — focus on the errors array first

**When to use:** After `run_project`, wait a moment, then call this to see what happened. Call multiple times if you need to wait for specific behavior to occur.

---

### stop_project

Stop the currently running Godot project and retrieve final output.

**Parameters:** None.

**Returns:** JSON object:

```json
{
  "message": "Project stopped",
  "finalOutput": ["line 1", ...],
  "finalErrors": ["error 1", ...]
}
```

**Gotchas:**
- Returns an error if no project is currently running
- The final output/errors include everything from the entire run, not just since the last `get_debug_output` call
- Always stop the project before running it again — `run_project` does kill existing processes, but explicit stop is cleaner

**When to use:** After you have captured enough output, or before re-running with a fix applied. Also call this to clean up when done debugging.

---

### launch_editor

Open the Godot editor for a project. Useful for manual inspection or when the agent needs the user to check something visually.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `projectPath` | string | yes | Path to directory containing `project.godot` |

**Returns:** Success message confirming the editor launched.

**Gotchas:**
- This opens the full Godot editor GUI — it's not headless
- The editor takes a few seconds to load; there's no way to query its state
- On headless servers, this will fail — it requires a display

**When to use:** Rarely needed in the debug-fix loop. Use when you need the user to visually inspect something that MCP tools can't capture, or to open the editor for a project that hasn't been imported yet.

---

## Project info tools

Use these to understand the project structure before or during debugging.

### get_project_info

Analyze a Godot project's structure.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `projectPath` | string | yes | Path to directory containing `project.godot` |

**Returns:** JSON object:

```json
{
  "name": "MyGame",
  "path": "/path/to/project",
  "godotVersion": "4.4",
  "structure": {
    "scenes": 12,
    "scripts": 24,
    "assets": 48
  }
}
```

**When to use:** To understand what's in a project before debugging, or to verify that expected files exist. Useful for "wrong node hierarchy" issues where you need to compare expected vs actual structure.

---

### get_godot_version

Check which Godot version is installed.

**Parameters:** None.

**Returns:** Version string (e.g. `"4.4.stable"`).

**When to use:** Before using UID tools (require 4.4+), or when debugging version-specific behavior.

---

### list_projects

Find Godot projects in a directory.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `directory` | string | yes | Directory to search |
| `recursive` | boolean | no | Search subdirectories (default: false) |

**Returns:** Array of objects:

```json
[
  { "path": "/path/to/project", "name": "MyGame" }
]
```

**When to use:** When you don't know the exact project path, or to verify which project to debug.

---

## Scene management tools

These tools modify project files. Use them to fix scene-level issues discovered during debugging — but prefer editing `.tscn` files directly for complex changes.

### create_scene

Create a new scene file with a root node.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `projectPath` | string | yes | Path to project directory |
| `scenePath` | string | yes | Where to save the scene, relative to project root (e.g. `scenes/player.tscn`) |
| `rootNodeType` | string | no | Root node type (e.g. `Node2D`, `Node3D`, `Control`). Defaults to `Node`. |

**Returns:** Success message.

---

### add_node

Add a node to an existing scene.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `projectPath` | string | yes | Path to project directory |
| `scenePath` | string | yes | Scene file path, relative to project |
| `nodeType` | string | yes | Type of node to add (e.g. `Sprite2D`, `CollisionShape2D`) |
| `nodeName` | string | yes | Name for the new node |
| `parentNodePath` | string | no | Path to parent node within the scene. Omit for root's child. |
| `properties` | object | no | Key-value pairs of properties to set on the node |

**Returns:** Confirmation with node name, type, and scene path.

---

### load_sprite

Load a texture into a Sprite2D, Sprite3D, or TextureRect node.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `projectPath` | string | yes | Path to project directory |
| `scenePath` | string | yes | Scene file path, relative to project |
| `nodePath` | string | yes | Path to the sprite node within the scene |
| `texturePath` | string | yes | Texture file path, relative to project |

**Returns:** Success message.

**Gotchas:**
- The target node must be a Sprite2D, Sprite3D, or TextureRect — other node types will fail
- Texture file must exist in the project directory

---

### save_scene

Save or resave a scene file, optionally to a new path (creating a variant).

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `projectPath` | string | yes | Path to project directory |
| `scenePath` | string | yes | Source scene file path |
| `newPath` | string | no | Save to a different path (creates a copy/variant) |

**Returns:** Confirmation with save location.

---

### export_mesh_library

Export a 3D scene as a MeshLibrary resource for use with GridMap.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `projectPath` | string | yes | Path to project directory |
| `scenePath` | string | yes | Path to `.tscn` file containing 3D meshes |
| `outputPath` | string | yes | Where to save the `.res` MeshLibrary file |
| `meshItemNames` | array | no | Specific mesh items to include (omit for all) |

**Returns:** Success message with output path.

---

## UID tools

Godot 4.4+ uses UIDs to track resources. These tools help manage them.

### get_uid

Get the UID for a specific file.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `projectPath` | string | yes | Path to project directory |
| `filePath` | string | yes | File to get UID for |

**Returns:** JSON with file path, absolute path, UID string, and exists flag.

**Gotchas:**
- Requires Godot 4.4+. Call `get_godot_version` first if unsure.
- Returns `exists: false` if the file has no UID (not all file types get UIDs).

---

### update_project_uids

Resave all resources in a project to regenerate and update UID references.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `projectPath` | string | yes | Path to project directory |

**Returns:** Success message with summary statistics (files processed, successes, failures).

**Gotchas:**
- This modifies files on disk — it resaves every scene and script in the project
- Requires Godot 4.4+
- Can take a while on large projects

**When to use:** After moving/renaming many files, or when UID references are broken (scenes fail to load resources they previously found).
