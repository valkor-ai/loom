---
name: godot-e2e
description: |
  Write and run E2E (end-to-end) game tests using the godot-e2e
  framework. Python controls a live Godot game over TCP — Locator-based
  semantic queries, expect() auto-retry assertions, and engine log
  capture make failures self-diagnosing.

  Use this skill whenever you need to:
  - Test actual gameplay: player movement, collisions, scoring, scene transitions
  - Verify UI interactions: button clicks, label text, menu navigation
  - Write integration tests that run the real game (not mocked unit tests)
  - Debug E2E test failures or set up E2E test infrastructure

  Triggers: "E2E test", "end-to-end test", "gameplay test", "test the game running",
  "simulate input", "test player movement", "test UI clicks", "godot-e2e",
  "integration test for game", "test scene transitions".
---

# godot-e2e — E2E Testing for Godot

$ARGUMENTS

godot-e2e is a custom framework with **zero LLM training data coverage**.
Everything the model needs is in this skill (with deeper detail in
`references/`). Do not guess — follow these docs exactly.

## Architecture

The `godot-e2e` CLI launches a Godot process and communicates over TCP
(localhost). Enabling the GodotE2E plugin in Project Settings
auto-registers an `AutomationServer` autoload that receives JSON
commands, executes them on the main thread, and sends back results.
The game runs unmodified — the server is dormant unless launched with
`--e2e`. Multiple instances can run in parallel (each auto-allocates a
unique port). The framework rests on three pillars: `Locator` for
semantic node queries, `expect()` for auto-retry assertions, and
engine log capture so every error carries the Godot logs that
preceded it.

## Quick Start — conftest.py + Test File

```python
# conftest.py (per test directory — explicit project path control;
# alternatively set GODOT_E2E_PROJECT_PATH env or pytest.ini
# `godot_e2e_project_path` and use the auto-registered `game` fixture).
# Replace "/root/Main" below with your project's entry-scene root —
# read it from `project.godot`'s `run/main_scene`.
import os

import pytest
from godot_e2e import GodotE2E

GODOT_PROJECT = os.path.join(os.path.dirname(__file__), "..")
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

```python
# test_player.py
from godot_e2e import expect

def test_player_moves_right(game):
    player = game.locator(group="player")          # Locator query
    initial_x = player.get_property("position:x")

    game.input_action("ui_right", True)
    game.wait_physics_frames(10)
    game.input_action("ui_right", False)

    expect(player).to_satisfy(
        lambda l: l.get_property("position:x") > initial_x,
        description="player moved right",
    )

def test_button_starts_game(game):
    game.get_by_button("Start").click()           # auto-waits actionability
    expect(game.locator(name="GameStatus")).to_have_text("Playing")
    errors = [e for e in game.collected_logs if e.level == "error"]
    assert not errors, f"errors during click: {errors}"
```

```bash
godot-e2e e2e/ -v
```

## API Quick Reference

### Launch / Lifecycle

| Method | Description |
|---|---|
| `GodotE2E.launch(project_path, godot_path=None, port=0, timeout=10.0, extra_args=None, log_verbosity=None)` | Context manager. Launch Godot + connect. `port=0` auto-allocates. `log_verbosity` ∈ `"error"`/`"warning"`/`"info"`. |
| `GodotE2E.connect(host="127.0.0.1", port=6008, token="")` | Connect to already-running Godot. |
| `game.close()` | Kill Godot process and close connection. |

### Locator — Semantic Queries

`Locator` is lazy: queries re-resolve on every action, so a Locator
created before `reload_scene()` still works after.

| Constructor | Description |
|---|---|
| `game.locator(path=, name=, group=, text=, type=, script=)` | At least one kwarg required; AND-composed. `name` / `text` accept glob (`*`, `?`). `type` matches via `is X` (descendants included, e.g. `type="BaseButton"` covers `Button`/`CheckBox`). |
| `game.get_by_text(text)` | Sugar for `locator(text=text)`. |
| `game.get_by_button(text)` | Sugar for `locator(type="BaseButton", text=text)`. |

| Refinement | Returns | Description |
|---|---|---|
| `loc.filter(**kwargs)` | `Locator` | Add AND-composed predicates. |
| `loc.first()` / `loc.nth(i)` | `Locator` | Pick first / i-th match. |
| `loc.all()` | `list[Locator]` | Snapshot of all matches; `[]` if none (no raise). |
| `loc.locator(**kwargs)` | `Locator` | Sub-query under this Locator's resolved node (parent resolved at action time). |

| Inspection (no raise on miss) | Returns |
|---|---|
| `loc.exists()` / `loc.count()` | `bool` / `int` |
| `loc.is_visible()` / `loc.is_actionable()` | `bool` (raises on multi-match / missing) |

| Action (re-resolves; requires exactly one match) | Notes |
|---|---|
| `loc.click(*, force=False, timeout=5.0)` | Auto-waits actionability for `Control` (visible + mouse_filter + in viewport); `Node2D` only checks visibility. `force=True` skips check. Raises `NotActionableError` on timeout. |
| `loc.hover()` | Inject `InputEventMouseMotion` at node's screen position. |
| `loc.get_property(prop)` / `loc.set_property(prop, value)` / `loc.call(method, args=None)` | Same as `game.*` versions, but path-pinned. |
| `loc.wait_visible(*, timeout=5.0)` | Block until target passes actionability. Raises `NotActionableError` with `reasons` + `checks`. |
| `loc.wait_for_signal(signal_name, timeout=5.0)` | Block until resolved node emits signal. |

### expect() — Auto-Retry Assertions

`expect(locator, *, timeout=5.0, poll_interval=0.05) → LocatorAssertions`
re-resolves the Locator on each poll. Lookup errors during polling
(`NodeNotFoundError`, `MultipleMatchesError`, `CommandError`) are
caught; node may appear / disambiguate later.

| Matcher | Passes When |
|---|---|
| `to_have_property(name, value)` | `locator.get_property(name) == value` |
| `to_have_text(text)` | Target's `text` property equals (sugar for property `"text"`). |
| `to_be_visible()` | Visible in scene tree (Control / Node2D). |
| `to_exist()` | Query resolves to ≥1 node. |
| `to_satisfy(predicate, *, description=None)` | `predicate(locator)` truthy. Use `description=` for readable failure messages. |

`ExpectationFailedError` dual-inherits `AssertionError` → pytest
renders it as a regular assertion failure, with attributes `actual`,
`observation_captured`, `matcher`, `scene_tree`, `last_error`.

### Engine Log Capture

| Member | Description |
|---|---|
| `game.last_logs` / `game.collected_logs` | List of `LogEntry`. `last_logs` cleared each command; `collected_logs` cleared per test by built-in fixtures. |
| `game.reset_collected_logs()` | Manual narrowing — reset window before a sub-assertion. |
| `game.set_log_verbosity(level)` / `game.set_log_buffer_size(size)` | Runtime tuning. Levels: `"error"` / `"warning"` (default) / `"info"`. |
| `LogEntry` fields | `level` / `message` / `function` / `file` / `line` (last three populated for engine errors only). |
| **Every `GodotE2EError` carries `.logs`** | List of LogEntry from the failing command's response. Empty when capture inactive. |

Pytest auto-includes `captured godot logs` section on failure (built-in
plugin, no setup required). Buffer overflow synthesizes a `"<N entries
dropped>"` warning.

### Raw-Path Operations

Direct on `game` — used when you have a stable known path and a
Locator query would just add ceremony (typically root-level
singletons, autoloads, the `Main` entry node). Node ops
(`game.get_property` / `set_property` / `call` / `find_by_group` /
`query_nodes` / `get_tree` / `batch`), input (`input_action` /
`input_key` / `input_mouse_*` / `press_action` / `press_key` /
`click` / `click_node`), waits (`wait_physics_frames` /
`wait_process_frames` / `wait_seconds` / `wait_for_node` /
`wait_for_signal` / `wait_for_property`), scenes (`get_scene` /
`change_scene` / `reload_scene` / `screenshot`).

### Types & Exceptions

```python
from godot_e2e import (
    Vector2, Vector2i, Vector3, Vector3i, Rect2, Rect2i,
    Color, Transform2D, NodePath,
    LogEntry, LogVerbosity, parse_log_entries,
    expect, Locator, LocatorAssertions,
)
```

| Exception | When |
|---|---|
| `NodeNotFoundError` | Node path doesn't exist. |
| `TimeoutError` | `wait_for_*` exceeded. Has `.scene_tree`. |
| `ConnectionLostError` | Godot crashed or TCP dropped. |
| `CommandError` | Server returned an error. |
| `MultipleMatchesError` | Locator action without `.first()`/`.nth()`/`.filter()` matched >1. Has `.paths`. |
| `NotActionableError` | `Locator.click()` / `wait_visible()` timed out waiting for actionability. Has `.path`, `.reasons`, `.checks`. |
| `ExpectationFailedError` | `expect(...)` matcher exceeded timeout. Dual-inherits `AssertionError`. |

All inherit from `GodotE2EError` (which carries `.logs`).

## Critical Rules

| # | Rule | Detail |
|---|------|--------|
| 1 | **Physics frames for movement** | After input, use `wait_physics_frames` for position/collision assertions. `wait_process_frames` does NOT advance physics. |
| 2 | **Hold input for movement** | `press_action` only taps (~4 frames). For sustained movement: `input_action(act, True)` → `wait_physics_frames(N)` → `input_action(act, False)`. |
| 3 | **`input_action` needs 2 args** | `input_action("jump", True)` not `input_action("jump")`. For tap, use `press_action("jump")`. |
| 4 | **Prefer `expect()` over manual wait + assert** | `expect(locator).to_have_property(...)` retries with structured failure context (scene_tree + last_error) and renders as a normal pytest assertion. Use `wait_for_property` only when working with raw paths and no Locator is in scope. |
| 5 | **Assert direction, not exact values** | `assert new_x > initial_x` not `assert pos_x == 450.0`. Physics varies per machine. |
| 6 | **`wait_for_signal` timing** | Listener registers on arrival — signals emitted before are missed. Use `expect().to_have_property` for state assertions. |
| 7 | **Locators with semantic queries beat hardcoded paths** | `game.get_by_button("Start")` / `game.locator(group="player")` survives tree restructuring; `/root/Main/UI/Menu/StartButton` doesn't. Reserve raw paths for unique top-level nodes. |
| 8 | **Read `.logs` on every E2E failure** | Every `GodotE2EError` carries `.logs` — what Godot printed during the failing command. `pytest -v` auto-includes `captured godot logs` on failure. Ignoring it doubles diagnosis time. |
| 9 | **Default log verbosity is `"warning"`** | `push_error` and `push_warning` are captured; `print()` is NOT. Bump to `"info"` (via `log_verbosity="info"` at launch or `game.set_log_verbosity("info")` at runtime) only when debugging — at info verbosity log buffer fills 4-10× faster. |
| 10 | **Use `wait_seconds` for Timer-gated waits** | `wait_process_frames(N)` counts frames, not seconds. Under headless uncapped FPS, `wait_process_frames(120)` finishes in well under 2s. Use `wait_seconds(t)` or `expect()` for any wait gated by a `Timer` or wall-clock seconds. |

## Fixture Strategies

| Strategy | Scope | Speed | Isolation | Use when |
|---|---|---|---|---|
| `reload_scene` | module process + function reload | Fast | Good | Default. Most tests. |
| `game_fresh` | function process | Slow | Maximum | Tests that modify global/autoload state. |
| `session` | session process | Fastest | None | Read-only tests, careful ordering. |

The pytest plugin auto-registers `game` (reload-based) and
`game_fresh` (process-per-test) — both reset `collected_logs` at test
entry and capture screenshots on failure to `test_output/`.

## Running & Debugging

```bash
godot-e2e e2e/ -v                                  # all tests
godot-e2e e2e/test_player.py -v                    # single file
godot-e2e --godot-path /path/to/godot tests/ -v    # specific binary
```

- **Engine log verbosity at launch**: `GodotE2E.launch(path, log_verbosity="info")` or `--e2e-log-verbosity=info` flag
- **Server-side wire log**: `extra_args=["--e2e-log"]` (separate from engine log capture — this logs request/response traffic on the Godot side)
- **TimeoutError diagnosis**: exception has `.scene_tree`
- **Locator actionability diagnosis**: `NotActionableError.reasons` lists failed checks (`"not_visible_in_tree"`, `"mouse_filter_ignore"`, `"outside_viewport"`, `"unclickable_node_type"`)
- **expect() failure context**: `ExpectationFailedError.actual` (last observed value) + `.scene_tree` + `.last_error` (last swallowed CommandError)

## E2E Test Quality Standards

Every E2E test MUST meet these minimum requirements. Tests that fail
these criteria are rejected.

### Minimum Per-Test Requirements

1. **At least 1 user action** — `input_action`, `press_action`, `click`, `Locator.click()`, or `call` that triggers gameplay
2. **At least 1 state-change assertion** — verify a property CHANGED (not just that a node exists). Prefer `expect(...)` matchers over manual `assert get_property(...) == X`.
3. **No pure existence tests** — `node_exists` / `Locator.exists()` may serve as a precondition, but NEVER as the only assertion

### Bad vs Good Examples

```python
# BAD — only checks existence, proves nothing about gameplay:
def test_player(game):
    assert game.locator(group="player").exists()

# GOOD — verifies actual gameplay behavior with auto-retry:
def test_player_moves_right(game):
    player = game.locator(group="player")
    initial_x = player.get_property("position:x")
    game.input_action("move_right", True)
    game.wait_physics_frames(10)
    game.input_action("move_right", False)
    expect(player).to_satisfy(
        lambda l: l.get_property("position:x") > initial_x,
        description="player moved right",
    )
```

## Fixture Sync Rules

1. **Entry scene change** → update `conftest.py`: change `wait_for_node` path, add `change_scene` if needed
2. **Entity naming change** → if helpers used hardcoded paths, update; if they used Locators with `group=` / `type=` / `text=`, often no change needed
3. **New game state** (e.g., menu before gameplay) → create a `game_playing` fixture that navigates past menus to gameplay state
4. **Private → public methods** → E2E `game.call()` / `Locator.call()` cannot call `_private()` methods; any method called by E2E must be public
5. **After ANY structural change** → run `godot-e2e e2e/ -v` to catch broken fixtures immediately


## Extended References

For full API details (every Locator method, every matcher's polling
semantics, all wire commands, full exception attribute lists, type
serialization tags):
→ Grep `references/api-reference.md`

For testing patterns (Locator-based UI recipes, expect() idioms,
log-driven diagnosis, keep-alive, pause handling, CI config, flaky
test mitigation):
→ Grep `references/testing-patterns.md`
