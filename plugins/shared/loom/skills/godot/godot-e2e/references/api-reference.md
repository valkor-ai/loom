# godot-e2e API Reference

Complete reference for all classes, methods, types, and exceptions.

## Table of Contents

1. [GodotE2E (main class)](#godote2e)
2. [Locator API](#locator-api)
3. [expect() — Auto-Retry Assertions](#expect--auto-retry-assertions)
4. [Engine Log Capture](#engine-log-capture)
5. [Node Operations](#node-operations)
6. [Input Simulation](#input-simulation)
7. [High-Level Input Helpers](#high-level-input-helpers)
8. [Frame Synchronization](#frame-synchronization)
9. [Synchronization (wait_for_*)](#synchronization)
10. [Scene Management](#scene-management)
11. [Screenshot](#screenshot)
12. [Batch Operations](#batch-operations)
13. [Types](#types)
14. [Exceptions](#exceptions)
15. [GodotClient (low-level)](#godotclient)
16. [GodotLauncher](#godotlauncher)
17. [pytest Fixtures](#pytest-fixtures)
18. [Godot Addon Setup](#godot-addon-setup)
19. [Wire Protocol Summary](#wire-protocol-summary)

---

## GodotE2E

`from godot_e2e import GodotE2E`

### GodotE2E.launch()

```python
GodotE2E.launch(
    project_path: str,             # Path to dir containing project.godot
    godot_path: str = None,        # Godot executable path (auto-discovered if None)
    port: int = 0,                 # TCP port (0 = auto-allocate free port)
    timeout: float = 10.0,         # Seconds to wait for connection
    extra_args: list = None,       # Extra args forwarded to Godot process
    log_verbosity: str = None,     # "error" / "warning" / "info"; None uses addon default ("warning")
) -> GodotE2E  # context manager
```

Launches Godot with `--e2e` flag, connects over TCP, completes handshake.
Use as context manager for automatic cleanup.

**Godot discovery order**: `godot_path` param > `GODOT_PATH` env var > PATH search
(`godot`, `godot4`, `Godot_v4`).

**Raises**: `FileNotFoundError` (no Godot), `RuntimeError` (Godot exits early),
`ConnectionError` (timeout), `ValueError` (invalid `log_verbosity`).

```python
with GodotE2E.launch("./my_project", timeout=15.0, log_verbosity="info") as game:
    game.wait_for_node("/root/Main", timeout=10.0)
    # ... tests ...
# Godot process killed automatically
```

### GodotE2E.connect()

```python
GodotE2E.connect(
    host: str = "127.0.0.1",
    port: int = 6008,
    token: str = ""
) -> GodotE2E
```

Connect to an already-running Godot instance (started manually with `--e2e`).
If token was set via `--e2e-token`, it must match.

```python
# Start Godot manually: godot --path ./project -- --e2e --e2e-port=6008
game = GodotE2E.connect(port=6008)
```

### close()

Terminate the Godot process (if launched) and close TCP connection.
Called automatically by context manager.

---

## Locator API

A `Locator` is a lazy reference to one or more nodes. Queries
re-resolve on every action, so a Locator created before
`reload_scene()` keeps working after.

### Construction

| Method | Signature | Returns |
|--------|-----------|---------|
| `game.locator(**kwargs)` | At least one of `path`, `name`, `group`, `text`, `type`, `script`. AND-composed. | `Locator` |
| `game.get_by_text(text)` | Sugar for `locator(text=text)`. | `Locator` |
| `game.get_by_button(text)` | Sugar for `locator(type="BaseButton", text=text)`. Covers `Button`, `CheckBox`, `OptionButton`, `MenuButton`, `LinkButton`. | `Locator` |

### Constructor Kwargs

| Kwarg | Type | Behavior |
|---|---|---|
| `path` | `str` | Absolute scene path (e.g., `"/root/Main/Player"`); resolves 0 or 1 match. |
| `name` | `str` | Node name. Glob (`*`, `?`) if value contains wildcards, else exact. |
| `group` | `str` | Godot group name; exact match. |
| `text` | `str` | Matched against `node.text` property. Same glob/exact rule as `name`. |
| `script` | `str` | Script resource path (e.g., `"res://player.gd"`); exact. |
| `type` | `str` | Class name; matched via `is X` (instanceof) — descendants included. |

### Refinement (returns new Locator; never mutates self)

| Method | Returns | Notes |
|---|---|---|
| `loc.filter(**kwargs)` | `Locator` | Add AND-composed predicates (same kwargs as `locator()`). |
| `loc.first()` | `Locator` | Always pick first match in tree-walk order. |
| `loc.nth(i: int)` | `Locator` | Pick i-th match (zero-indexed). Raises `ValueError` if `i < 0`. |
| `loc.all()` | `list[Locator]` | Resolve immediately; returns path-pinned Locator per match. `[]` if no match (no raise). Snapshot at call time. |
| `loc.locator(**kwargs)` | `Locator` | Sub-query scoped under this Locator's resolved node. Parent is re-resolved on every action of the chained Locator. Parent must resolve to exactly one node at action time; `MultipleMatchesError` / `NodeNotFoundError` raised when action runs (not on method call). `exists()` / `count()` swallow these and return `False` / `0`. |

### Inspection (safe from multi/zero match)

| Method | Returns | Raises | Notes |
|---|---|---|---|
| `loc.exists()` | `bool` | — | True if query resolves to ≥1 node. Never raises on lookup issues; connection errors still propagate. |
| `loc.count()` | `int` | — | Number of matching nodes. |
| `loc.is_visible()` | `bool` | `MultipleMatchesError`, `NodeNotFoundError` | Whether single-match target is visible in scene tree (uses `is_visible_in_tree`). |
| `loc.is_actionable()` | `bool` | `MultipleMatchesError`, `NodeNotFoundError` | Whether single-match target passes all actionability checks. |

### Action (re-resolves; requires exactly one match)

| Method | Signature | Notes / Raises |
|---|---|---|
| `loc.click(*, force=False, timeout=5.0)` | `None` | For `Control`: polls actionability up to `timeout`, raises `NotActionableError` if never passes. For `Node2D`: only checks `is_visible_in_tree()`. For `Node3D` / `Window` / `Node`: fails with `unclickable_node_type`. `force=True` skips check entirely. |
| `loc.hover()` | `None` | Inject `InputEventMouseMotion` at node's screen position. Triggers `mouse_entered` / `_gui_input` on intervening Controls. |
| `loc.get_property(prop)` | (deserialized) | Same semantics as `game.get_property`. Sub-property paths like `"position:x"` supported. |
| `loc.set_property(prop, value)` | `None` | Same semantics as `game.set_property`. Python type wrappers auto-serialized. |
| `loc.call(method, args=None)` | (deserialized) | Same semantics as `game.call`. |
| `loc.wait_visible(*, timeout=5.0)` | `None` | Block until target passes actionability checks. Identical checks to `click()` auto-wait. Raises `NotActionableError` with structured `reasons` (list of: `"not_visible_in_tree"`, `"mouse_filter_ignore"`, `"outside_viewport"`, `"unclickable_node_type"`) and `checks` (per-check booleans). |
| `loc.wait_for_signal(signal_name, timeout=5.0)` | `list` | Signal arguments. Raises `TimeoutError`, `CommandError` (signal doesn't exist). |

All action methods raise `MultipleMatchesError` (with `paths` attribute)
when query matches multiple nodes without `.first()` / `.nth()` /
`.filter()`, and `NodeNotFoundError` when zero match (except
`.all()`, `.exists()`, `.count()`).

### Resolution Semantics

- Each action method re-runs the full query against the live scene tree immediately before executing. This keeps Locators valid across `reload_scene()` and tree mutations.
- `filter(**kwargs)` predicates AND with primary query.
- Chained Locators (`.locator(**kwargs)`): parent resolved at action time; parent resolution errors propagate (not swallowed like `.all()`).

### Examples

```python
# Single button by visible text
game.get_by_button("Start").click()

# Player by group
player = game.locator(group="player")
expect(player).to_have_property("health", 100)

# Multiple enemies with explicit indexing
enemies = game.locator(group="enemies").all()
for enemy in enemies:
    enemy.call("take_damage", [10])

# Disambiguate with filter
game.locator(type="Button").filter(text="OK").click()

# Sub-query — find a specific child by name within the player
overlay = game.locator(group="player").locator(name="HealthBar")
```

---

## expect() — Auto-Retry Assertions

Playwright-style polling assertions over a Locator. Each matcher
polls until it passes or times out.

### Constructor

```python
expect(
    locator: Locator,
    *,
    timeout: float = 5.0,
    poll_interval: float = 0.05,
) -> LocatorAssertions
```

| Parameter | Default | Notes |
|---|---|---|
| `locator` | required | Re-resolved on every poll. |
| `timeout` | `5.0` | Seconds. Validates `>= 0`. |
| `poll_interval` | `0.05` | Seconds between polls (50 ms). Validates `> 0`. |

**Raises**: `TypeError` (locator is not a `Locator`), `ValueError` (invalid timeout / poll_interval).

### Matchers

All return `None` on success. Raise `ExpectationFailedError` on timeout.

| Matcher | Signature | Passes When |
|---|---|---|
| `to_have_property(name, value)` | — | `locator.get_property(name) == value`. |
| `to_have_text(text)` | — | Target's `text` property equals `text`. Sugar for `to_have_property("text", text)`. Works on `Label`, `Button`, `LineEdit`, `RichTextLabel`, etc. (all expose under same name). |
| `to_be_visible()` | — | Target visible in scene tree (Control / Node2D). For `Node3D` / `Window` / `Node`, use `to_satisfy(lambda l: l.get_property("visible"))`. |
| `to_exist()` | — | Locator's query resolves to ≥1 node. Does not require exactly one match. |
| `to_satisfy(predicate, *, description=None)` | predicate: `Callable[[Locator], Any]` | `predicate(locator)` returns truthy. Lookup errors inside predicate (`NodeNotFoundError`, `MultipleMatchesError`, `CommandError`) are caught and treated as "not yet satisfied". Use `description=` for human-readable failure messages (else `repr(predicate)` is used — useless for lambdas). |

### Polling Error Handling

| Exception during poll | Behavior |
|---|---|
| `NodeNotFoundError` | Caught, loop keeps polling (node may appear later). |
| `MultipleMatchesError` | Caught, loop keeps polling. |
| `CommandError` | Caught, stashed as `last_error` for failure message; loop keeps polling. |
| Other exceptions | Propagate immediately (real bugs, not transient states). |

### ExpectationFailedError Attributes

```python
class ExpectationFailedError(GodotE2EError, AssertionError):
    actual: Any                    # last observed value (None if never observed)
    observation_captured: bool     # True if at least one poll succeeded
    matcher: str                   # e.g., "to_have_text('Ready')"
    scene_tree: dict | None        # depth-4 dump of /root at failure time; None on dump failure
    timeout: float                 # the timeout that was exceeded, in seconds
    last_error: Exception | None   # last CommandError swallowed during polling, if any
    logs: list[LogEntry]           # via GodotE2EError base
```

**Dual-inheritance**: Inherits both `GodotE2EError` and `AssertionError`,
so pytest renders it as a regular assertion failure (in the assertion
section of the report, not as a generic exception traceback).

### Examples

```python
from godot_e2e import expect

# Property equality
expect(game.locator(group="player")).to_have_property("health", 100)

# Text assertion with shorter timeout
expect(game.locator(name="StatusLabel"), timeout=2.0).to_have_text("Ready")

# Visibility
expect(game.locator(name="GameOverPanel")).to_be_visible()

# Existence (≥1 match — count NOT required to be 1)
expect(game.locator(group="enemies")).to_exist()

# Custom predicate with description
expect(game.locator(group="player")).to_satisfy(
    lambda l: l.get_property("position:x") > 500.0,
    description="player crossed x=500",
)
```

---

## Engine Log Capture

A Godot-side `Logger` subclass intercepts engine log events
(`push_error`, `push_warning`, runtime errors, shader errors) plus
optional `print()` / `printerr()` at info verbosity. Events are
returned in every command's response under `_logs`, parsed into
`LogEntry` objects by the Python client.

### LogEntry

```python
@dataclass
class LogEntry:
    level: str       # "error" | "warning" | "info" | "stderr"
    message: str
    function: str    # populated only for engine errors (_log_error callback)
    file: str        # populated only for engine errors
    line: int        # populated only for engine errors
```

| Field | Populated When |
|---|---|
| `level` | always |
| `message` | always |
| `function` | engine errors only (empty for `info` / `stderr`) |
| `file` | engine errors only (empty for `info` / `stderr`) |
| `line` | engine errors only (0 for `info` / `stderr`) |

`LogEntry.__str__()` renders as `[LEVEL] message (file:line)` for use
in failure reports.

### LogVerbosity

```python
from godot_e2e import LogVerbosity

LogVerbosity.ERROR    # "error"   — only push_error / runtime / shader errors
LogVerbosity.WARNING  # "warning" — + push_warning  [DEFAULT]
LogVerbosity.INFO     # "info"    — + print() / printerr()
```

### Properties / Methods

| Member | Description |
|---|---|
| `game.last_logs -> list[LogEntry]` | Engine log entries captured during the most recent command. Cleared on each new command. |
| `game.collected_logs -> list[LogEntry]` | All entries since last reset. Pytest fixtures reset at test entry, so under standard `game` / `game_fresh` fixtures this list reflects the current test only (including its scene reload). On test failure, appended to pytest report under `captured godot logs` section alongside `captured stdout` / `captured stderr`. |
| `game.reset_collected_logs()` | Discard `collected_logs` and `last_logs`. Pytest fixtures call automatically; manual use only when scoping a sub-assertion to a narrower window. |
| `game.set_log_verbosity(level: str)` | Adjust capture verbosity at runtime. `level` ∈ `"error"`, `"warning"`, `"info"`. Raises `CommandError` for invalid values. |
| `game.set_log_buffer_size(size: int)` | Resize ring buffer at runtime. Default 200. Raises `ValueError` (Python boundary) if `size < 1`; `CommandError` (wire boundary) for other invalid values. Use to raise for high-error-density debug sessions where entries drop, or shrink to validate overflow handling. |
| `parse_log_entries(raw: list) -> list[LogEntry]` | Convert raw `_logs` array (as it arrives on the wire) into list. Exposed for callers bypassing the high-level API. |

### Startup Configuration

| Flag / Kwarg | Default | Notes |
|---|---|---|
| `--e2e-log-verbosity=<level>` (CLI) | `"warning"` | Set at Godot startup. Passed via `GodotLauncher.launch()`. |
| `log_verbosity=<level>` (`GodotE2E.launch()` kwarg) | `None` (defers to addon default) | Validated and raises `ValueError` before subprocess starts. |

### Buffer Overflow

When the ring buffer overflows between drains (more entries emitted
than buffer size allows), the response carries a `_logs_dropped`
count. The Python client synthesizes a single warning entry with
message `"<N log entries dropped due to capture buffer overflow>"`
and appends it uniformly to:
- `last_logs`
- `collected_logs`
- Any raised exception's `.logs`

### Exception Log Attachment

Every `GodotE2EError` subclass has a `.logs` attribute (list of
`LogEntry`) populated from the failing command's `_logs` payload.
Empty list when log capture inactive.

### Example

```python
from godot_e2e import GodotE2E, expect

with GodotE2E.launch("./my_project", log_verbosity="info") as game:
    game.wait_for_node("/root/Main", timeout=10.0)

    # Run game logic
    game.get_by_button("Start").click()
    expect(game.locator(name="Status")).to_have_text("Playing")

    # Assert no errors during the click
    errors = [e for e in game.collected_logs if e.level == "error"]
    assert not errors, f"errors during click: {[str(e) for e in errors]}"

    # Narrow to a single command
    game.reset_collected_logs()
    game.call("/root/Main", "trigger_dangerous_thing")
    assert any(e.message == "expected warning" for e in game.last_logs), \
        f"missing expected warning; got: {game.last_logs}"
```

---

## Node Operations

### node_exists(path) -> bool

Check if a node exists in the scene tree.

```python
game.node_exists("/root/Main/Player")  # True or False
```

### get_property(path, property) -> value

Get a node property. Supports Godot's colon-separated sub-property notation.

| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | `str` | Absolute node path: `"/root/Main/Player"` |
| `property` | `str` | Property name. Use `:` for sub-properties: `"position:x"` |

**Returns**: Deserialized Python value (float, str, Vector2, etc.).

**Raises**: `NodeNotFoundError`, `CommandError` (property doesn't exist).

```python
pos = game.get_property("/root/Main/Player", "position")       # Vector2
x = game.get_property("/root/Main/Player", "position:x")       # float
text = game.get_property("/root/Main/Label", "text")            # str
visible = game.get_property("/root/Main/Menu", "visible")       # bool
health = game.get_property("/root/Main/Player", "health")       # int
```

### set_property(path, property, value)

Set a property on a node. Use godot-e2e type classes for Godot types.

```python
from godot_e2e import Vector2

game.set_property("/root/Main/Player", "position", Vector2(100.0, 200.0))
game.set_property("/root/Main/Player", "position:x", 500.0)
game.set_property("/root/Main", "score", 0)
game.set_property("/root/Main/Label", "text", "Hello")
```

### call(path, method, args=None) -> value

Call a GDScript method on a node.

| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | `str` | Node path |
| `method` | `str` | Method name |
| `args` | `list` | Arguments list (each is serialized) |

```python
result = game.call("/root/Main", "get_counter")           # No args
game.call("/root/Main", "add_to_counter", [5])             # With args
game.call("/root/Main", "reset_level", [True, 3])          # Multiple args
```

### find_by_group(group) -> list[str]

Find all nodes in a Godot group. Returns list of absolute path strings.

```python
enemies = game.find_by_group("enemies")
# ["/root/Main/Enemy1", "/root/Main/Enemy2"]
```

### query_nodes(pattern="", group="") -> list[str]

Query nodes by glob pattern (`*`, `?` wildcards), group, or both.
**Note**: For new code, prefer `game.locator(...)` — Locator wraps
this with a richer query DSL and re-resolution.

```python
game.query_nodes(pattern="Enemy*")                    # All nodes named Enemy*
game.query_nodes(group="enemies")                     # All in "enemies" group
game.query_nodes(pattern="Boss*", group="enemies")    # Bosses in enemies group
```

### get_tree(path="/root", depth=4) -> dict

Scene tree snapshot as nested dict. Keys: `name`, `type`, `path`, `children`.

```python
tree = game.get_tree("/root/Main", depth=2)
# {"name": "Main", "type": "Node2D", "path": "/root/Main",
#  "children": [
#    {"name": "Player", "type": "CharacterBody2D", ...},
#    {"name": "Label", "type": "Label", ...}
#  ]}
```

---

## Input Simulation

All input commands are **deferred**: the server injects the input event,
waits 2 physics frames for Godot to process it, then responds.

### input_action(action_name, pressed, strength=1.0)

Simulate a named input action (defined in Godot's Input Map).

**Most reliable method** — focus-independent. Use for all gameplay actions.

**Limitation**: Does NOT drive `Input.get_axis()` / `Input.get_vector()`.
If game code uses these, use `input_key()` instead.

```python
game.input_action("ui_right", True)    # press
game.input_action("ui_right", False)   # release
game.input_action("jump", True, strength=0.5)  # partial strength
```

### input_key(keycode, pressed, physical=False)

Simulate a keyboard key event. Goes through Godot's full input pipeline
(`_input`, `_unhandled_input`, action mapping). **DOES drive `Input.get_axis()`**.

| Parameter | Type | Description |
|-----------|------|-------------|
| `keycode` | `int` | Godot key constant (e.g., `4194321` = KEY_RIGHT) |
| `pressed` | `bool` | True for key-down, False for key-up |
| `physical` | `bool` | If True, sets physical_keycode (layout-independent) |

Common Godot keycodes (decimal):
- KEY_LEFT: 4194319, KEY_RIGHT: 4194321, KEY_UP: 4194320, KEY_DOWN: 4194322
- KEY_SPACE: 32, KEY_ENTER: 4194309, KEY_ESCAPE: 4194305
- KEY_A: 65, KEY_D: 68, KEY_W: 87, KEY_S: 83

### input_mouse_button(x, y, button=1, pressed=True)

Mouse button at screen coordinates. Button: 1=left, 2=right, 3=middle.

### input_mouse_motion(x, y, relative_x=0, relative_y=0)

Mouse motion event at screen position with optional relative movement.

---

## High-Level Input Helpers

Convenience wrappers that handle press + release automatically.

### press_action(action_name, strength=1.0)

Press and immediately release a named action. Equivalent to:
`input_action(action, True)` then `input_action(action, False)`.
Total wait: 4 physics frames (2 per input_action call).

**Use for taps/clicks, NOT for held movement.** For held input, use
`input_action(name, True)` → `wait_physics_frames(N)` → `input_action(name, False)`.

### press_key(keycode)

Press and release a key. Equivalent to two `input_key` calls.

### click(x, y, button=1)

Click at screen coordinates. Mouse down + wait + mouse up.

### click_node(path)

Click at a node's screen position. The server computes coordinates:
- **Control nodes**: center of `get_global_rect()`
- **Node2D nodes**: viewport-transformed global position

```python
game.click_node("/root/Menu/StartButton")  # Click the button
```

**Raises**: `NodeNotFoundError`, `CommandError` (unsupported node type).

**For new code, prefer `Locator.click()`** — it auto-waits actionability
(visible + mouse_filter + in viewport) and surfaces structured failure
reasons via `NotActionableError`.

---

## Frame Synchronization

### wait_process_frames(count=1)

Wait N `_process` (render) frames. Use for UI animations, `_process` logic.

### wait_physics_frames(count=1)

Wait N `_physics_process` frames. **Use for movement, collision, physics.**

### wait_seconds(seconds)

Wait N in-game seconds. Affected by `Engine.time_scale`.
Timeout parameters use wall-clock time (NOT affected by time_scale).

---

## Synchronization

### wait_for_node(path, timeout=5.0)

Block until node exists. Polls every process frame on the Godot side (fast).

**Raises**: `TimeoutError` with `.scene_tree` attribute containing a tree dump.

```python
game.wait_for_node("/root/Level2", timeout=10.0)
```

### wait_for_signal(path, signal_name, timeout=5.0)

Wait for a signal to emit. Returns list of signal arguments.

**IMPORTANT**: Only catches signals emitted AFTER the command is received.
For state changes triggered by actions, prefer `expect(locator).to_have_property(...)`.

```python
args = game.wait_for_signal("/root/Main", "level_complete", timeout=10.0)
```

### wait_for_property(path, property, value, timeout=5.0)

Poll until property equals expected value. Polls on Godot side (fast, no
network round-trips per poll).

**For new code, prefer `expect(locator).to_have_property(...)`** — it
adds Locator re-resolution and richer failure context. Use this only
when working with hardcoded paths and the assertion is downstream of
non-Locator code.

```python
game.wait_for_property("/root/Main", "score", 10, timeout=5.0)
```

---

## Scene Management

### get_scene() -> str

Returns current scene's `res://` path.

### change_scene(scene_path)

Change to a new scene. **Blocks until new scene is loaded and ready.**

```python
game.change_scene("res://levels/level2.tscn")
# No need for wait_for_node — change_scene already waits
```

### reload_scene()

Reload the current scene. **Blocks until reloaded.** Primary test isolation
mechanism — resets all scene state.

---

## Screenshot

### screenshot(save_path="") -> str

Capture viewport to PNG. Returns absolute file path.

If `save_path` is empty, saves to `user://e2e_screenshots/<timestamp>.png`.

The built-in pytest fixtures auto-capture on test failure to `test_output/`.

---

## Batch Operations

### batch(commands) -> list

Execute multiple **instant** commands in one TCP round-trip.

Each command is either a dict with `"action"` key, or a tuple of
`(action, params_dict)`.

**Deferred commands (input, waits) are NOT supported in batch** — they return
an error entry.

```python
results = game.batch([
    ("get_property", {"path": "/root/Main/Player", "property": "position:x"}),
    ("get_property", {"path": "/root/Main/Player", "property": "position:y"}),
    ("get_property", {"path": "/root/Main", "property": "score"}),
    {"action": "node_exists", "path": "/root/Main/Enemy"},
])
x, y, score, enemy_exists = results
```

---

## Types

`from godot_e2e import Vector2, Vector2i, Vector3, Vector3i, Rect2, Rect2i, Color, Transform2D, NodePath`

All are Python dataclasses mirroring Godot types. Used for `set_property`
values and returned by `get_property`.

| Type | Wire Tag | Fields |
|------|----------|--------|
| `Vector2(x, y)` | `"v2"` | `float, float` |
| `Vector2i(x, y)` | `"v2i"` | `int, int` |
| `Vector3(x, y, z)` | `"v3"` | `float, float, float` |
| `Vector3i(x, y, z)` | `"v3i"` | `int, int, int` |
| `Rect2(x, y, w, h)` | `"r2"` | `float, float, float, float` |
| `Rect2i(x, y, w, h)` | `"r2i"` | `int, int, int, int` |
| `Color(r, g, b, a=1.0)` | `"col"` | `float, float, float, float` |
| `Transform2D(x, y, origin)` | `"t2d"` | `Vector2, Vector2, Vector2` |
| `NodePath(path)` | `"np"` | `str` |

Wire protocol uses `_t` type tags for lossless JSON round-trip.
Unsupported Godot types become `{"_t": "_unknown", "_class": "...", "_str": "..."}`.

---

## Exceptions

All inherit from `GodotE2EError`. **Every instance carries `.logs`**
(list of `LogEntry`) populated from the failing command's response.

```python
from godot_e2e import (
    GodotE2EError,
    NodeNotFoundError,
    TimeoutError,
    ConnectionLostError,
    CommandError,
    MultipleMatchesError,
    NotActionableError,
    ExpectationFailedError,
)
```

### NodeNotFoundError

Node path doesn't exist in scene tree.
Raised by: `get_property`, `set_property`, `call`, `click_node`,
`wait_for_signal`, Locator actions.

### TimeoutError

Wait operation exceeded timeout. Has `.scene_tree` attribute (dict or None)
with a tree dump captured at timeout.

```python
try:
    game.wait_for_node("/root/Missing", timeout=2.0)
except TimeoutError as e:
    print(e.scene_tree)  # Shows what nodes DO exist
```

### ConnectionLostError

Godot process crashed or TCP connection dropped. Raised by any command.

### CommandError

Server returned an error (unknown command, bad property, failed method call).

### MultipleMatchesError

Locator action matched multiple nodes without `.first()` / `.nth()` /
`.filter()` to disambiguate.

| Attribute | Type | Description |
|---|---|---|
| `paths` | `list[str]` | All matching node paths. |

```python
try:
    game.locator(type="Button").click()
except MultipleMatchesError as e:
    print(f"matched {len(e.paths)} buttons: {e.paths}")
    # Disambiguate:
    game.locator(type="Button").filter(text="OK").click()
```

### NotActionableError

`Locator.click()` or `Locator.wait_visible()` timed out waiting for
the target to become actionable.

| Attribute | Type | Description |
|---|---|---|
| `path` | `str` | Resolved node path. |
| `reasons` | `list[str]` | Failed checks, drawn from: `"not_visible_in_tree"`, `"mouse_filter_ignore"`, `"outside_viewport"`, `"unclickable_node_type"`. |
| `checks` | `dict[str, bool]` | Per-check booleans for full visibility into actionability state. |

### ExpectationFailedError

Raised by `expect(locator).to_*` matchers on timeout. Dual-inherits
`GodotE2EError` and `AssertionError`, so pytest renders it as a
regular assertion failure.

| Attribute | Type | Description |
|---|---|---|
| `actual` | `Any` | Last observed value (or `None` if never observed). |
| `observation_captured` | `bool` | True if at least one poll succeeded and returned a value. |
| `matcher` | `str` | Human-readable matcher name (e.g., `"to_have_text('Ready')"`). |
| `scene_tree` | `dict \| None` | Depth-4 dump of `/root` at failure time; `None` on dump failure. |
| `timeout` | `float` | The timeout that was exceeded, in seconds. |
| `last_error` | `Exception \| None` | Last `CommandError` swallowed during polling, if any. |

---

## GodotClient

`from godot_e2e import GodotClient`

Low-level TCP client. You normally use `GodotE2E` instead.

- `GodotClient(host, port)` — constructor
- `connect(timeout=10.0)` — open TCP connection
- `close()` — close connection
- `hello(token)` — send handshake
- `send_command(action, **params)` — send command and block for response

---

## GodotLauncher

`from godot_e2e import GodotLauncher`

Process manager. Used internally by `GodotE2E.launch()`.

- `launch(project_path, godot_path, port, timeout, extra_args, log_verbosity=None)` — start Godot, return connected client
- `kill()` — gracefully shut down Godot (quit command → terminate → kill)

The launcher:
1. Finds Godot binary
2. If `port=0` (default), creates a temporary port file and passes
   `--e2e-port=0 --e2e-port-file=<path>` so Godot auto-selects a free port
   and writes it to the file. This avoids TOCTOU race conditions and enables
   multiple parallel instances.
3. Generates random authentication token
4. Starts Godot with `--e2e`, `--e2e-port=N`, `--e2e-token=X`,
   `--e2e-port-file` (when auto-allocating), and
   `--e2e-log-verbosity=<level>` (when `log_verbosity` is set)
5. Reads actual port from port file (if auto-allocated), then polls until
   TCP connection + handshake succeeds

Validates `log_verbosity` and raises `ValueError` before subprocess starts
for invalid values.

---

## pytest Fixtures

The `godot_e2e.fixtures` module registers as a pytest plugin via
`pytest11` entry point — fixtures available in any test without
`pytest_plugins` declaration.

### Built-in `game` fixture (function scope)

Backed by a module-scoped Godot process. Reloads scene before each test.
Auto-resets `collected_logs` at test entry. Auto-captures screenshot on
test failure to `test_output/<test_name>_failure.png`.

Project path resolution order:
1. `@pytest.mark.godot_project("path")` marker
2. `godot_e2e_project_path` in pytest config (`pytest.ini` / `pyproject.toml`)
3. `GODOT_E2E_PROJECT_PATH` env var
4. Auto-detection of `project.godot` in `./godot_project`, `../godot_project`, `.`

### Built-in `game_fresh` fixture (function scope)

Fresh Godot process per test. Maximum isolation, slowest. Same auto-reset
and auto-screenshot behavior.

### Pytest report integration

On test failure, the plugin appends `collected_logs` under the
`captured godot logs` section of the report — alongside `captured stdout`
and `captured stderr`. No setup required.

### Custom fixtures (recommended for pipeline)

Write your own `conftest.py` for full control:

```python
import os

import pytest
from godot_e2e import GodotE2E

GODOT_PROJECT = os.path.join(os.path.dirname(__file__), "..", "godot_project")
GODOT_PATH = os.environ.get("GODOT_PATH")

@pytest.fixture(scope="module")
def _game_process():
    with GodotE2E.launch(
        GODOT_PROJECT,
        godot_path=GODOT_PATH,
        timeout=15.0,
    ) as game:
        game.wait_for_node("/root/Main", timeout=10.0)
        yield game

@pytest.fixture(scope="function")
def game(_game_process):
    _game_process.reload_scene()
    _game_process.wait_for_node("/root/Main", timeout=5.0)
    yield _game_process
```

---

## Godot Addon Setup

### Files to copy

Copy `addons/godot_e2e/` into your Godot project. It contains:

| File | Purpose |
|------|---------|
| `plugin.gd` | EditorPlugin: auto-registers `AutomationServer` autoload |
| `plugin.cfg` | Addon metadata |
| `automation_server.gd` | Autoload: TCP server + state machine |
| `command_handler.gd` | Command dispatch + execution |
| `json_serializer.gd` | GDScript <-> JSON type conversion |
| `config.gd` | CLI flag parser (`--e2e`, `--e2e-port`, etc.) |
| `logger.gd` | `Logger` subclass for engine log capture |

### Plugin registration

Enable in **Project > Project Settings > Plugins** — check the **GodotE2E** entry.
The plugin automatically adds `AutomationServer` as an autoload (no
manual `project.godot` edit needed).

### CLI flags (passed after `--` separator)

| Flag | Description |
|------|-------------|
| `--e2e` | Enable automation server. Required. |
| `--e2e-port=N` | TCP port (default: 6008). Use `0` for auto-selection. |
| `--e2e-port-file=PATH` | Write actual port to this file. Used with `--e2e-port=0` for parallel instances. |
| `--e2e-token=X` | Auth token (must match client). |
| `--e2e-log` | Verbose **server-side wire log** to stdout (separate from engine log capture — this logs request/response on Godot side). |
| `--e2e-log-verbosity=<level>` | Engine log capture verbosity at startup. `"error"` / `"warning"` / `"info"`. |

The launcher passes these automatically. For manual launch:
```bash
godot --path ./project -- --e2e --e2e-port=6008 --e2e-log --e2e-log-verbosity=info
```

### Zero production impact

The AutomationServer checks `--e2e` in `_ready()`. Without it:
- No TCP server is created
- `set_process(false)` + `set_physics_process(false)`
- Zero runtime overhead

---

## Wire Protocol Summary

Every response includes a `_logs` array (empty if log capture inactive
or no logs emitted). The Python client parses this into `LogEntry`
objects and populates `last_logs`, `collected_logs`, and exception
`.logs` attributes.

### Locator-related commands

| Command | Params | Response | Notes |
|---|---|---|---|
| `find_nodes` | `query: dict` (with `by`, `value`, optional `filters`), `start_path: str` | `{"nodes": [...], "_logs": [...]}` | Multi-strategy node lookup. Backs all `Locator` queries. |
| `node_actionable` | `path: str` | `{"actionable": bool, "checks": {"visible": bool, "mouse_filter": bool, "in_viewport": bool}, "reasons": [<failed_checks>], "_logs": [...]}` | Snapshot of node actionability. |
| `hover_node` | `path: str` | (deferred) | Inject `InputEventMouseMotion` at node's screen position. |

### Log capture commands

| Command | Params | Response | Notes |
|---|---|---|---|
| `set_log_verbosity` | `level: str` | (empty dict + `_logs`) | Runtime verbosity adjustment. |
| `set_log_buffer_size` | `size: int` | (empty dict + `_logs`) | Runtime ring buffer resize. |

### Direct-path commands

| Command | Params | Notes |
|---|---|---|
| `get_property` / `set_property` / `call` / `node_exists` / `find_by_group` / `query_nodes` / `get_tree` / `batch` | (see sections above) | Instant. |
| `click_node` | `path: str` | Deferred. Used by `Locator.click()`. |
| `input_action` / `input_key` / `input_mouse_button` / `input_mouse_motion` | (see Input Simulation) | Deferred. |
| `wait_for_node` / `wait_for_signal` / `wait_for_property` / `wait_physics_frames` / `wait_process_frames` / `wait_seconds` | (see Synchronization) | Deferred. |
| `change_scene` / `reload_scene` / `get_scene` | (see Scene Management) | Deferred (scene loads), instant (`get_scene`). |
| `screenshot` | `save_path: str` | Instant. |
