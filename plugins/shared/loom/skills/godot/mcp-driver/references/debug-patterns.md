# Common Runtime Debug Patterns

Patterns the agent should recognize when analyzing MCP debug output, and how to fix them.

## Quick reference

| Pattern | How you detect it | Typical root cause | Fix approach |
|---|---|---|---|
| Null reference | `SCRIPT ERROR: Invalid get index` or `Attempt to call on a null instance` in errors | Node path wrong, node not ready, freed node | Check node paths, use `@onready`, add null guard |
| Nothing visible | No errors, but game window shows nothing | No camera, wrong viewport, z-index, offscreen | Check Camera2D/3D exists and is `current`, check node positions |
| Physics passthrough | Objects overlap or fall through | Wrong collision layer/mask, missing shape | Check collision_layer and collision_mask bits, verify CollisionShape2D has a shape |
| Input not responding | No errors, but input produces no effect | Missing Input Map action, wrong action name, input not connected | Check Project Settings > Input Map, verify action names in code match |
| Signal not connected | Expected behavior never triggers | Signal not connected, wrong signal name, connected to wrong method | Check signal connections in scene and code, verify signal name spelling |
| Infinite loop / freeze | Output stops, no new lines from get_debug_output | While loop without exit, _process doing too much | Check loops in _process/_physics_process, add break conditions |
| Scene load failure | `Failed to load resource` in errors | Missing file, wrong path, circular dependency | Verify file exists at the res:// path, check for typos |
| Type error at runtime | `Invalid type in function` in errors | Wrong argument type passed to function | Check function signatures, verify variable types |
| Autoload crash | Errors appear immediately on startup, before any game logic | Autoload script has error in _ready() | Check autoload scripts listed in project.godot |
| Missing export vars | Node exists but behaves with default values | Export vars not set in editor / scene file | Check .tscn file for the node's property values |

## Detailed patterns

### Null reference at runtime

**Detection:** errors array contains messages like:
- `Invalid get index 'position' (on base: 'null instance')`
- `Attempt to call function 'add_child' in base 'null instance'`
- `SCRIPT ERROR: ... Attempt to call ...`

**Analysis steps:**
1. Find the script and line number from the error message
2. Identify which variable is null
3. Check how that variable gets its value — `@onready`, `get_node()`, `$NodePath`, or assignment

**Common causes:**
- Node path is wrong (typo, wrong hierarchy)
- Accessing a node in `_init()` or property initializer — too early, scene tree not ready yet
- Node was freed by another system (use `is_instance_valid()` to check)
- Dynamically created node not added to tree yet

**Fix:** Use `@onready var node = $Path` instead of `var node = $Path`. For dynamic lookups, add `if node != null:` or `if is_instance_valid(node):`.

---

### Nothing visible (black screen)

**Detection:** `run_project` succeeds, `get_debug_output` shows no errors, but the game window shows nothing.

**Analysis steps:**
1. Use `get_project_info` to check if scenes exist
2. Check if a main scene is set in `project.godot`
3. Look for Camera2D/Camera3D in the scene tree
4. Check if nodes have non-zero size/scale and are at visible positions

**Common causes:**
- No Camera2D (2D game) or Camera3D (3D game) in the scene, or camera's `current` property is false
- Sprites/nodes positioned far offscreen (position at 0,0 in a 3D scene might be behind the camera)
- CanvasLayer or viewport misconfiguration hiding content
- Node `visible` property set to false
- Z-index ordering: everything drawn behind something opaque

**Fix:** Ensure a Camera node exists with `current = true`. Verify node positions are within the viewport. Check `visible` and `z_index` properties.

---

### Physics not working

**Detection:** Objects pass through each other, fall through floors, or don't collide.

**Analysis steps:**
1. Check for collision-related errors in output
2. Verify collision layers/masks in the scene files
3. Ensure CollisionShape2D/3D nodes have actual shapes assigned

**Common causes:**
- Collision layer and mask don't overlap — layer is "what I am", mask is "what I collide with". Both sides need matching bits.
- CollisionShape2D exists but has no `shape` resource assigned (the shape is null)
- Using `move_and_slide()` without setting velocity first
- StaticBody2D used where CharacterBody2D was needed (or vice versa)
- Physics bodies as children of other physics bodies — causes unexpected behavior

**Fix:** Cross-reference with the **physics reviewer skill** for detailed collision layer/mask guidance. The most common fix: set body A's `collision_mask` to include body B's `collision_layer` bit, and vice versa.

---

### Input not responding

**Detection:** No errors, game runs, but pressing keys/buttons does nothing.

**Analysis steps:**
1. Check if the Input Map has the expected action names (look at `project.godot` for `[input]` section)
2. Search code for the action names used in `Input.is_action_pressed()` etc.
3. Verify the action names in code match exactly (case-sensitive)

**Common causes:**
- Action not defined in Project Settings > Input Map
- Typo in action name: code says `"move_right"` but Input Map has `"MoveRight"`
- Using `_input()` instead of `_unhandled_input()` and something is consuming the event first
- Node's `process_mode` set to `PROCESS_MODE_DISABLED`
- For UI: `mouse_filter` on a Control node is consuming events before they reach the game

**Fix:** Verify action names match exactly between code and Input Map. If actions are missing, add them to `project.godot` or create via editor.

---

### Signal not connected

**Detection:** No errors, but expected behavior (damage dealing, score updating, state change) never triggers.

**Analysis steps:**
1. Check if the signal is defined on the emitting node
2. Check if it's connected — in the .tscn file (connection section) or in code (`connect()`)
3. Verify the method signature matches what the signal emits

**Common causes:**
- Signal connected in code but the connect line never executes (wrong branch, early return)
- Signal name misspelled (no error for connecting to a non-existent signal in GDScript unless using typed signals)
- Connected to a method with wrong parameter count
- Emitting node was freed and recreated, losing the connection

**Fix:** Add `print("signal fired")` to verify emission. Check both `.tscn` `[connection]` section and code-based connections. Use `node.is_connected("signal_name", callable)` to verify at runtime.

---

### Infinite loop / freeze

**Detection:** `get_debug_output` returns the same output on repeated calls — no new lines appearing. Or the output suddenly stops after a specific point.

**Analysis steps:**
1. Note the last output line — the freeze happened right after that point
2. Find the corresponding code location
3. Look for while loops, recursive calls, or heavy _process logic

**Common causes:**
- `while` loop without proper exit condition
- Recursive function without base case
- `_process` or `_physics_process` doing expensive work every frame
- `await` on a signal that never fires (hangs that coroutine, but doesn't freeze the whole game unless it's blocking main)

**Fix:** Add iteration limits to while loops. Move expensive operations out of per-frame callbacks. Add timeouts to await calls. Use `stop_project` to kill the frozen process before applying fixes.

---

### Autoload crash on startup

**Detection:** Errors appear in `get_debug_output` immediately after `run_project`, before any game logic can run. Errors reference scripts listed in `[autoload]` section of `project.godot`.

**Analysis steps:**
1. Read `project.godot` to find autoload entries
2. Check each autoload script for errors in `_ready()` or `_init()`
3. Autoloads initialize in order — if the first one crashes, later ones may not load

**Common causes:**
- Autoload references a node that doesn't exist yet (other autoloads not ready)
- File path in autoload config points to a moved/deleted script
- Autoload _ready() calls an API that requires the scene tree to be fully ready

**Fix:** Check autoload order dependencies. Use `call_deferred()` for operations that need other autoloads to be ready first.
