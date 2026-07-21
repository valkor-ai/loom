# Optional Godot Dependencies

Addons are owned by the Godot project. Loom only coordinates their use and
records the resulting verification evidence.

## Before installing an addon

1. Read the addon's supported Godot versions.
2. Check whether the project uses GDScript or .NET.
3. Pin the compatible release instead of tracking an arbitrary branch.
4. Install only the documented addon directory under `addons/`.
5. Confirm `plugin.cfg` and any required autoload entries are present.

Keep downloaded source archives outside the project or in a temporary directory
that is ignored by Git. Do not commit editor caches, `.godot/`, or generated
reports unless the project explicitly treats them as source assets.

## Common verification

```bash
GODOT_BIN="${GODOT_PATH:-godot}"
"$GODOT_BIN" --headless --path . --editor --quit
```

If an addon fails to load, check the Godot version and the addon entry in
`project.godot` before changing application code.
