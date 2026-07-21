---
name: project-scaffold
description: |
  Inspect or initialize a Godot project without imposing a game architecture.
  Use when starting a project, checking project.godot, adding a main scene,
  or deciding which optional Godot addons are needed.
---

# Godot Project Scaffold

Loom does not prescribe an ECS, folder layout, asset pipeline, or game genre.
Keep the existing project structure when one is already present. For a new
project, use the smallest structure that matches the request and record the
choice in the Loom architecture or task contract.

## Inspect before editing

From the directory containing `project.godot`, check:

```bash
test -f project.godot
rg -n '^(config/features|run/main_scene|config/name|renderer/|environment/defaults)' project.godot
find . -maxdepth 2 -type f \( -name '*.tscn' -o -name '*.gd' -o -name '*.cs' \) | sort
```

Read the configured main scene before creating another entry point. Reuse
existing autoloads, input actions, renderer settings, and naming conventions.

## Minimal new project layout

Godot's Project Manager can create the project. A small code-first project can
start with:

```text
project-root/
├── project.godot
├── scenes/
│   └── main.tscn
├── scripts/
├── assets/
└── tests/                 # only when tests are part of the project
```

For a 3D project, use a `Node3D` root, a `Camera3D`, at least one light, and a
world environment or clear background. Add collision shapes alongside bodies;
do not rely on visual meshes alone for gameplay collision.

## project.godot checks

Use the exact ConfigFile syntax already present in the project. The common
sections are:

```ini
[application]
config/name="Project Name"
run/main_scene="res://scenes/main.tscn"

[display]
window/size/viewport_width=1280
window/size/viewport_height=720

[rendering]
renderer/rendering_method="gl_compatibility"
```

Only add `[input]`, `[autoload]`, physics, or renderer settings when the task
needs them. Confirm every script-referenced input action exists before handing
off the task.

## Optional dependencies

Treat addons as project dependencies, not as Loom features. Check the addon's
own compatibility table and pin a version that matches the installed Godot
version. Do not copy an entire addon repository into `addons/`; install only
the addon directory and verify that it contains `plugin.cfg` where applicable.

The `references/project_settings.md` file contains the detailed ConfigFile and
input serialization patterns carried over from the Godot reference material.

## Verification

After scaffolding or changing scenes:

```bash
GODOT_BIN="${GODOT_PATH:-godot}"
"$GODOT_BIN" --headless --path . --editor --quit
```

Then run the project's tests, launch the main scene, and capture evidence when
the task includes runtime or visual acceptance criteria.
