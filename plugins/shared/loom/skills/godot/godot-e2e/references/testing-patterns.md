# godot-e2e Testing Patterns and Best Practices

## Table of Contents

1. [Fixture Strategies](#fixture-strategies)
2. [Physics-Based Testing](#physics-based-testing)
3. [UI Testing](#ui-testing)
4. [Locator-Based UI Testing](#locator-based-ui-testing)
5. [expect()-Driven Assertions](#expect-driven-assertions)
6. [Log Capture for Debugging](#log-capture-for-debugging)
7. [State Verification](#state-verification)
8. [Scene Transition Testing](#scene-transition-testing)
9. [Screenshot on Failure](#screenshot-on-failure)
10. [Flaky Test Mitigation](#flaky-test-mitigation)
11. [Batch Operations for Performance](#batch-operations-for-performance)
12. [Debugging Tips](#debugging-tips)
13. [CI Configuration](#ci-configuration)
14. [Common Gotchas](#common-gotchas)

---

## Fixture Strategies

### Strategy 1: Scene Reload (default, recommended)

One Godot process per test module. Scene reloaded before each test.

```python
@pytest.fixture(scope="module")
def _game_process():
    with GodotE2E.launch(PROJECT_PATH, timeout=15.0) as game:
        game.wait_for_node("/root/Main", timeout=10.0)
        yield game

@pytest.fixture(scope="function")
def game(_game_process):
    _game_process.reload_scene()
    _game_process.wait_for_node("/root/Main", timeout=5.0)
    yield _game_process
```

**When to use**: Most tests. Resets scene tree, node properties, script variables.
**Limitation**: Global state (singletons, autoloads, static vars) persists between tests.

### Strategy 2: Fresh Process (maximum isolation)

```python
@pytest.fixture(scope="function")
def game_fresh():
    with GodotE2E.launch(PROJECT_PATH, timeout=15.0) as game:
        game.wait_for_node("/root/Main", timeout=10.0)
        yield game
```

**When to use**: Tests modifying global state, crash recovery tests.
**Cost**: ~2-5 seconds per test for Godot startup.

### Strategy 3: Shared Session (fastest)

```python
@pytest.fixture(scope="session")
def game_session():
    with GodotE2E.launch(PROJECT_PATH, timeout=15.0) as game:
        game.wait_for_node("/root/Main", timeout=10.0)
        yield game
```

**When to use**: Read-only tests, carefully ordered tests.
**Risk**: No reset between tests. A crash ends the session.

### Skip the main menu

Jump directly to the scene under test:

```python
@pytest.fixture(scope="module")
def _game_process():
    with GodotE2E.launch(PROJECT_PATH) as game:
        game.wait_for_node("/root", timeout=10.0)
        game.change_scene("res://levels/level1.tscn")
        game.wait_for_node("/root/Level1", timeout=5.0)
        yield game
```

### Reset to a specific scene in function fixture

```python
@pytest.fixture(scope="function")
def game(_game_process):
    current = _game_process.get_scene()
    if not current.endswith("menu.tscn"):
        _game_process.change_scene("res://menu.tscn")
    else:
        _game_process.reload_scene()
    _game_process.wait_for_node("/root/Menu", timeout=5.0)
    yield _game_process
```

---

## Physics-Based Testing

### Always use wait_physics_frames for movement

```python
def test_player_moves_right(game):
    initial_x = game.get_property("/root/Main/Player", "position:x")
    game.input_action("ui_right", True)
    game.wait_physics_frames(10)          # NOT wait_process_frames
    game.input_action("ui_right", False)
    new_x = game.get_property("/root/Main/Player", "position:x")
    assert new_x > initial_x
```

### When to use which wait

| Wait | Use for |
|------|---------|
| `wait_physics_frames` | CharacterBody2D movement, collision, RigidBody, `is_on_floor()` |
| `wait_process_frames` | Animation progress, UI transitions, `_process` logic |
| `wait_seconds` | Timed game events, cooldowns (game time, not wall time) |
| `wait_for_property` | Any state that will eventually change (preferred over frame counts) |

### Gravity / falling test

```python
def test_player_falls(game):
    initial_y = game.get_property("/root/Main/Player", "position:y")
    game.wait_physics_frames(30)
    new_y = game.get_property("/root/Main/Player", "position:y")
    assert new_y > initial_y  # Y increases downward in Godot
```

### Jump test

```python
def test_player_jumps(game):
    # Ensure on ground first
    game.wait_for_property("/root/Main/Player", "is_on_floor", True, timeout=3.0)
    initial_y = game.get_property("/root/Main/Player", "position:y")
    game.press_action("jump")
    game.wait_physics_frames(5)
    peak_y = game.get_property("/root/Main/Player", "position:y")
    assert peak_y < initial_y  # Y decreases upward
```

---

## UI Testing

### Click a node (recommended)

```python
def test_button_click(game):
    game.click_node("/root/Menu/StartButton")
    game.wait_for_node("/root/GameLevel", timeout=5.0)
```

### Verify label text after click

```python
def test_click_updates_label(game):
    game.click_node("/root/Menu/ClickButton")
    game.wait_process_frames(2)
    text = game.get_property("/root/Menu/StatusLabel", "text")
    assert "Clicked" in text
```

### Navigate between scenes via UI

```python
def test_navigate_to_detail_and_back(game):
    game.click_node("/root/Menu/NavigateButton")
    game.wait_for_node("/root/Detail", timeout=5.0)

    game.click_node("/root/Detail/BackButton")
    game.wait_for_node("/root/Menu", timeout=5.0)
    assert game.get_property("/root/Menu/TitleLabel", "text") == "Main Menu"
```

### Check visibility

```python
def test_pause_menu_visibility(game):
    assert game.get_property("/root/Main/PauseMenu", "visible") == False
    game.press_action("ui_cancel")
    game.wait_process_frames(5)
    assert game.get_property("/root/Main/PauseMenu", "visible") == True
```

> **For new code, prefer the Locator + `expect()` patterns below.**
> They survive scene-tree restructuring (no hardcoded paths) and
> replace manual `wait + assert` with structured retries. The
> direct-API patterns above remain valid for code working from
> stable raw paths (root-level singletons, autoloads).

---

## Locator-Based UI Testing

`Locator` is lazy and re-resolves on every action. Use semantic
queries (`group=`, `type=`, `text=`) instead of hardcoded paths so
tests survive minor scene refactors.

### Click a button by visible text

```python
def test_start_button_starts_game(game):
    game.get_by_button("Start Game").click()  # auto-waits actionability
    expect(game.locator(name="GameStatus")).to_have_text("Playing")
```

`get_by_button(text)` covers `Button`, `CheckBox`, `OptionButton`,
`MenuButton`, `LinkButton` (anything `is BaseButton`).
`Locator.click()` polls `is_visible_in_tree` + `mouse_filter` +
viewport intersect for `Control` nodes; raises `NotActionableError`
with structured `reasons` if never actionable.

### Disambiguate ambiguous queries

```python
# RAW — would raise MultipleMatchesError on a screen with multiple buttons:
# game.locator(type="Button").click()

# Pick by index:
game.locator(type="Button").first().click()
game.locator(type="Button").nth(2).click()

# Or filter:
game.locator(type="Button").filter(text="OK").click()
```

### Iterate matched nodes

```python
def test_all_enemies_take_damage(game):
    enemies = game.locator(group="enemies").all()
    assert len(enemies) > 0, "no enemies in scene"
    for enemy in enemies:
        before = enemy.get_property("health")
        enemy.call("take_damage", [10])
        assert enemy.get_property("health") == before - 10
```

`.all()` returns a snapshot of path-pinned `Locator` instances —
subsequent tree mutations don't update the list. Returns `[]` (no
raise) when nothing matches.

### Sub-queries (chained Locators)

```python
# Find a child by name within the player
overlay = game.locator(group="player").locator(name="HealthBar")
expect(overlay).to_be_visible()
```

The parent (`group="player"`) must resolve to exactly one node at
action time — `MultipleMatchesError` propagates from action calls.
`exists()` and `count()` swallow these and return `False` / `0`.

### Wait for actionability without clicking

```python
def test_overlay_appears(game):
    panel = game.locator(name="GameOverPanel")
    game.set_property("/root/Main/Player", "health", 0)
    panel.wait_visible(timeout=3.0)  # raises NotActionableError on timeout
```

`NotActionableError.reasons` lists which checks failed:
`"not_visible_in_tree"`, `"mouse_filter_ignore"`,
`"outside_viewport"`, `"unclickable_node_type"`.

### When to keep raw paths

Top-level singletons that aren't expected to move:
```python
game.wait_for_node("/root/Main", timeout=5.0)
game.wait_for_property("/root/Main", "score", 10, timeout=5.0)
```

Reserve raw paths for `wait_for_node` (waiting for the entry-point
scene), root-level autoloads, and one-off debugging. For all
gameplay assertions, use Locator + `expect()`.

---

## expect()-Driven Assertions

`expect(locator, *, timeout=5.0, poll_interval=0.05)` re-resolves the
Locator on every poll. Lookup errors during polling
(`NodeNotFoundError`, `MultipleMatchesError`, `CommandError`) are
caught — the node may appear / disambiguate later.

### Property equality

```python
expect(game.locator(group="player")).to_have_property("health", 100)
```

### Text assertion (sugar for `to_have_property("text", text)`)

```python
expect(game.locator(name="StatusLabel"), timeout=2.0).to_have_text("Ready")
```

### Visibility (Control / Node2D only)

```python
expect(game.locator(name="GameOverPanel")).to_be_visible()
```

For `Node3D` / `Window` / `Node`, fall back to `to_satisfy`:
```python
expect(game.locator(name="Cube3D")).to_satisfy(
    lambda l: l.get_property("visible"),
    description="Cube3D visible",
)
```

### Existence (≥1 match — count NOT required to be 1)

```python
expect(game.locator(group="enemies")).to_exist()
```

### Custom predicates with description

```python
def test_player_crosses_threshold(game):
    player = game.locator(group="player")
    initial_x = player.get_property("position:x")

    game.input_action("ui_right", True)
    game.wait_physics_frames(20)
    game.input_action("ui_right", False)

    expect(player, timeout=3.0).to_satisfy(
        lambda l: l.get_property("position:x") > initial_x + 100,
        description=f"player moved past x={initial_x + 100}",
    )
```

`description=` is the human-readable matcher name in failure
messages. Without it, the matcher reports `repr(predicate)`, which is
useless for lambdas. Always pass `description=` for `to_satisfy`.

### Per-call timeout override

```python
# Default 5.0s
expect(panel).to_be_visible()

# Tight timeout for fast-changing UI
expect(label, timeout=0.5).to_have_text("Ready")

# Long timeout for level-load assertions
expect(game.locator(name="Level2"), timeout=15.0).to_exist()
```

### When `expect()` fails — diagnosing `ExpectationFailedError`

```python
from godot_e2e import ExpectationFailedError

try:
    expect(game.locator(name="StatusLabel"), timeout=2.0).to_have_text("Ready")
except ExpectationFailedError as e:
    print(f"matcher={e.matcher} actual={e.actual!r} observed={e.observation_captured}")
    print(f"timeout={e.timeout}")
    if e.last_error is not None:
        print(f"last swallowed CommandError: {e.last_error}")
    if e.scene_tree is not None:
        import json
        print("scene tree at failure:", json.dumps(e.scene_tree, indent=2))
    if e.logs:
        print("godot logs during polling:", [str(x) for x in e.logs])
    raise
```

`ExpectationFailedError` dual-inherits `AssertionError`, so pytest
renders it in the assertion section of the report (not as a generic
exception traceback).

### `expect()` vs `wait_for_property + assert`

```python
# Direct-API form — fine when you only have a raw path
game.wait_for_property("/root/Main/Player", "health", 0, timeout=3.0)
assert game.get_property("/root/Main/Player", "health") == 0

# Locator form — single retry-with-context
expect(game.locator(group="player"), timeout=3.0).to_have_property("health", 0)
```

The Locator form re-resolves on each poll (so works after
`reload_scene()`), provides `scene_tree` + `last_error` in the
failure message, and renders as a normal pytest assertion. Pick
based on whether you have a Locator handle in scope, not by default.

---

## Log Capture for Debugging

Every `GodotE2EError` carries `.logs` (list of `LogEntry`) populated
from the failing command's response. Pytest auto-includes a
`captured godot logs` section on failure — no setup needed.
`game.collected_logs` is reset by built-in fixtures at test entry, so
it reflects the current test only.

### Assert no errors during a test

```python
def test_button_click_quietly(game):
    game.get_by_button("OK").click()
    expect(game.locator(name="Status")).to_have_text("done")

    errors = [e for e in game.collected_logs if e.level == "error"]
    assert not errors, f"unexpected errors: {[str(e) for e in errors]}"
```

### Assert a specific warning was raised

```python
def test_dangerous_call_warns(game):
    game.reset_collected_logs()  # narrow window to this command
    game.call("/root/Main", "trigger_dangerous_thing")

    warning_msgs = [e.message for e in game.last_logs if e.level == "warning"]
    assert any("about to do dangerous thing" in m for m in warning_msgs), \
        f"missing expected warning; got: {warning_msgs}"
```

`last_logs` is cleared on every command — use it for assertions about
a single command. Use `collected_logs` for whole-test assertions.

### Bump verbosity to capture print() output

```python
from godot_e2e import GodotE2E

with GodotE2E.launch("./project", log_verbosity="info") as game:
    # ... `print()` and `printerr()` from gameplay are now captured
    game.call("/root/Main", "noisy_method")
    info_lines = [e for e in game.last_logs if e.level == "info"]
    assert any("expected info" in e.message for e in info_lines)
```

Default verbosity `"warning"` captures `push_error` and `push_warning`
only. `"info"` adds `print()` (level `"info"`) and `printerr()` (level
`"stderr"`); buffer fills 4-10× faster.

### Runtime verbosity changes

```python
def test_specific_section(game):
    game.set_log_verbosity("info")
    try:
        game.call("/root/Main", "verbose_section")
        # assert on info-level logs
    finally:
        game.set_log_verbosity("warning")
```

### Inspect logs from a failed exception

```python
from godot_e2e import GodotE2EError

def test_handles_missing_node(game):
    try:
        game.set_property("/root/MissingNode", "health", 0)
    except GodotE2EError as e:
        # Even on lookup failure, e.logs may have engine warnings
        engine_warnings = [x for x in e.logs if x.level == "warning"]
        assert not engine_warnings or "deprecated" not in engine_warnings[0].message
        raise
```

### Buffer overflow

If many errors fire between drains, the ring buffer (default 200)
overflows. The Python client synthesizes a single warning entry:
```
LogEntry(level="warning", message="<N log entries dropped due to capture buffer overflow>", ...)
```
appended to `last_logs`, `collected_logs`, and exception `.logs`.
Resize at runtime with `game.set_log_buffer_size(1000)`.

### When to NOT log-assert

Log assertions are powerful but can be brittle if the engine's log
phrasing changes. Don't anchor tests on:
- exact GDScript runtime error strings (Godot version-sensitive)
- log line counts (timing-sensitive)
- warnings from third-party addons (gecs, gdUnit4, etc. may emit
  routine warnings unrelated to your code)

Anchor instead on:
- presence/absence of `level == "error"` entries (the generic "no
  errors during this test" assertion is the sweet spot)
- specific game-code messages (matched by substring, not exact equality)
- warnings tied to your own `push_warning("foo")` calls

---

## State Verification

### Prefer wait_for_property over polling

```python
# BAD: manual polling loop
for _ in range(100):
    game.wait_physics_frames(1)
    if game.get_property("/root/Main", "score") == 10:
        break
else:
    assert False, "Score never reached 10"

# GOOD: server-side polling (fast, no network round-trips per poll)
game.wait_for_property("/root/Main", "score", 10, timeout=5.0)
```

### Test coin collection via teleportation

```python
def test_coin_increases_score(game):
    initial_score = game.get_property("/root/Main", "score")
    coin_pos = game.get_property("/root/Main/Coin", "global_position")
    game.set_property("/root/Main/Player", "global_position", coin_pos)
    game.wait_for_property("/root/Main", "score", initial_score + 1, timeout=2.0)
```

### Test method call return values

```python
def test_increment_method(game):
    result = game.call("/root/Main", "increment")
    assert result == 1
    assert game.get_property("/root/Main", "counter") == 1
```

---

## Scene Transition Testing

### change_scene blocks until loaded

```python
def test_level_transition(game):
    game.change_scene("res://levels/level2.tscn")
    # change_scene already waits for the scene to load
    game.wait_for_node("/root/Level2", timeout=5.0)  # extra safety for child init
    name = game.get_property("/root/Level2", "level_name")
    assert name == "Level 2"
```

### reload resets state

```python
def test_reload_resets(game):
    game.call("/root/Main", "add_to_counter", [10])
    assert game.get_property("/root/Main", "counter") == 10

    game.reload_scene()
    game.wait_for_node("/root/Main", timeout=5.0)
    assert game.get_property("/root/Main", "counter") == 0
```

### Verify current scene

```python
scene = game.get_scene()
assert "level2.tscn" in scene
```

---

## Screenshot on Failure

### Automatic (built-in fixtures)

Both `game` and `game_fresh` built-in fixtures auto-capture screenshots on failure.
Saved to `test_output/<test_name>_failure.png`.

### Manual capture

```python
def test_visual_state(game):
    game.press_action("ui_accept")
    path = game.screenshot("/tmp/after_accept.png")
    assert os.path.isfile(path)
```

### CI artifact collection (GitHub Actions)

```yaml
- name: Upload failure screenshots
  if: failure()
  uses: actions/upload-artifact@v4
  with:
    name: e2e-failure-screenshots
    path: test_output/
```

---

## Flaky Test Mitigation

### Rule 1: State-based over time-based

```python
# FLAKY: frame count depends on machine
game.press_action("ui_accept")
game.wait_physics_frames(5)
assert game.get_property("/root/Main", "animation_done") == True

# STABLE: waits until condition met
game.press_action("ui_accept")
game.wait_for_property("/root/Main", "animation_done", True, timeout=5.0)
```

### Rule 2: Direction over exact values

```python
# FRAGILE: exact position varies per machine
assert game.get_property(player, "position:x") == 450.0

# ROBUST: direction is deterministic
assert new_x > initial_x
```

### Rule 3: Generous timeouts

```python
game.wait_for_node("/root/Main", timeout=10.0)    # 10s for initial load
game.wait_for_property("/root/Main", "ready", True, timeout=5.0)
```

### Rule 4: Expose game state as properties

Instead of inferring state from position, add script variables:
`is_on_ground`, `is_dead`, `current_level`, `is_paused`.

---

## Batch Operations for Performance

```python
# SLOW: 3 TCP round-trips
x = game.get_property(player, "position:x")
y = game.get_property(player, "position:y")
health = game.get_property(player, "health")

# FAST: 1 TCP round-trip
results = game.batch([
    ("get_property", {"path": player, "property": "position:x"}),
    ("get_property", {"path": player, "property": "position:y"}),
    ("get_property", {"path": player, "property": "health"}),
])
x, y, health = results
```

Only instant commands work in batch. Deferred (input, waits) return errors.

---

## Debugging Tips

### Enable server-side logging

```python
with GodotE2E.launch(path, extra_args=["--e2e-log"]) as game:
    ...
```

Logs every request/response on the Godot side:
```
[godot-e2e] << get_property (id=2)
[godot-e2e] >> {"id":2,"result":{"_t":"v2","x":400.0,"y":300.0}}
```

### Dump scene tree

```python
import json
tree = game.get_tree("/root", depth=3)
print(json.dumps(tree, indent=2))
```

### TimeoutError diagnosis

```python
try:
    game.wait_for_node("/root/Missing", timeout=2.0)
except TimeoutError as e:
    print("Scene tree at timeout:", json.dumps(e.scene_tree, indent=2))
```

### Interactive debugging

```bash
# Terminal 1: Start Godot in E2E mode
godot --path ./project -- --e2e --e2e-port=6008 --e2e-log

# Terminal 2: Connect from Python
python -c "
from godot_e2e import GodotE2E
game = GodotE2E.connect(port=6008)
print(game.get_tree('/root', depth=2))
game.close()
"
```

---

## CI Configuration

### Windows (GitHub Actions)

```yaml
- name: Install Godot
  shell: pwsh
  run: |
    Invoke-WebRequest -Uri "https://github.com/godotengine/godot-builds/releases/download/4.4-stable/Godot_v4.4-stable_win64.exe.zip" -OutFile godot.zip
    Expand-Archive godot.zip -DestinationPath C:\godot

- name: Run E2E tests
  run: godot-e2e e2e/ -v --timeout=60
  env:
    GODOT_PATH: C:\godot\Godot_v4.4-stable_win64.exe
```

### Linux (GitHub Actions, Xvfb required)

```yaml
- name: Install Godot
  run: |
    wget -q https://github.com/godotengine/godot-builds/releases/download/4.4-stable/Godot_v4.4-stable_linux.x86_64.zip
    unzip -q Godot_v4.4-stable_linux.x86_64.zip
    sudo mv Godot_v4.4-stable_linux.x86_64 /usr/local/bin/godot

- name: Run E2E tests
  run: xvfb-run --auto-servernum godot-e2e e2e/ -v --timeout=60
```

### macOS (GitHub Actions)

```yaml
- name: Run E2E tests
  run: godot-e2e e2e/ -v --timeout=60
  env:
    GODOT_PATH: /Applications/Godot.app/Contents/MacOS/Godot
```

### CI tips

- Increase `timeout` in `GodotE2E.launch()` to 15s for CI (first launch is slow)
- Upload `test_output/` as artifact for failure screenshots
- Add `--timeout=60` to godot-e2e / pytest to catch frozen Godot
- On Linux: use `--rendering-driver opengl3` if Vulkan not available
- godot-e2e does NOT support `--headless` (Godot bug #73557)

---

## Game State Survival Patterns

Games with death, pause, or scene reload mechanics can break E2E tests. These patterns prevent the most common failures.

### Keep-alive: prevent game-over during tests

If the player can die from inaction (Flappy Bird, platformers), long `wait_seconds` or `wait_physics_frames` calls will kill the player and crash the test (scene reload breaks the TCP connection).

```python
def keep_alive(game, frames, action="flap", interval=15):
    """Wait N physics frames while periodically pressing an action to stay alive."""
    elapsed = 0
    while elapsed < frames:
        chunk = min(interval, frames - elapsed)
        game.wait_physics_frames(chunk)
        elapsed += chunk
        if elapsed < frames:
            game.press_action(action)

def test_pipe_spawning(game):
    # WRONG: game.wait_seconds(3.0)  — bird dies, scene reloads, connection lost
    # RIGHT: keep alive while waiting
    keep_alive(game, 120, action="flap", interval=10)
    # Now check pipe count
```

### Smart keep-alive: read game state to decide actions

```python
def smart_keep_alive(game, frames, player_path, action="flap"):
    """Keep alive by monitoring player position — don't flap into the ceiling."""
    for _ in range(frames):
        game.wait_physics_frames(1)
        y = game.get_property(player_path, "position:y")
        if y > 400:  # too low, about to hit ground
            game.press_action(action)
        # If y < 100, don't flap — too high
```

### Pause handling: avoid input_action deadlock

`input_action` internally waits 2 physics frames. If the action triggers a pause (setting `get_tree().paused = true`), physics frames stop, and `input_action` hangs forever.

```python
# WRONG: deadlocks if "pause" triggers get_tree().paused = true
game.input_action("pause", True)

# RIGHT: use call() to toggle pause via method
game.call("/root/Main", "toggle_pause")
game.wait_process_frames(2)  # process frames still run when paused

# RIGHT: if you must use input, set AutomationServer to always process
# (In your Godot project code):
# AutomationServer.process_mode = Node.PROCESS_MODE_ALWAYS
```

### Scene reload: avoid reload_current_scene in gameplay

If game-over calls `get_tree().reload_current_scene()`, the ECS autoload state may not reset properly, and the TCP connection may break.

**Mitigation in test code:**
```python
# Use change_scene instead of reload_scene to force a clean load
game.change_scene("res://main.tscn")
game.wait_for_node("/root/Main", timeout=5.0)
```

**Mitigation in game code (worker brief should specify):**
- Game over → show UI overlay, don't reload scene
- Restart → `change_scene_to_file()` instead of `reload_current_scene()`
- ECS: call `ECS.world.clear()` before scene transition

---

## Common Gotchas

### 1. press_action vs held input

`press_action("move_right")` only taps (press + release = ~4 physics frames).
For movement that needs sustained input, hold explicitly:

```python
game.input_action("move_right", True)   # hold down
game.wait_physics_frames(20)             # hold for 20 frames
game.input_action("move_right", False)  # release
```

### 2. input_action vs input_key

`input_action` does NOT drive `Input.get_axis()` / `Input.get_vector()`.
If CharacterBody2D code uses these, use `input_key` with the mapped scancode.

### 3. Signal timing with wait_for_signal

Signal listener registers when the command arrives. Signals emitted BEFORE
the command are missed. Use `wait_for_property` for state-change assertions.

### 4. Exact position assertions

Physics produces different results across machines. Assert direction or ranges:
```python
assert new_x > old_x  # direction
assert abs(pos.x - expected) < 5.0  # range
```

### 5. Global state leaking between tests

`reload_scene` does NOT reset autoload singletons. If tests modify global state,
use `game_fresh` fixture (fresh process per test).

### 6. --headless not supported

godot-e2e requires a display. On Linux CI, use `xvfb-run`. On Windows/macOS CI,
a desktop session is available by default.

### 7. Batch limitations

`batch()` only supports instant commands. Any deferred command (input, wait_*,
change_scene) in a batch returns an error entry.

### 8. Timeout vs game time

`wait_seconds(t)` waits game time (affected by `Engine.time_scale`).
All `timeout` parameters use wall-clock time (not affected by time_scale).
If game sets `time_scale=0.1`, `wait_seconds(5)` takes 50 real seconds,
but `timeout=5.0` still fires after 5 real seconds.
