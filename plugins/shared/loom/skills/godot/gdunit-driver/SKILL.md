---
name: gdunit-driver
description: |
  Run gdUnit4 unit tests and parse results into structured output.
  Use this skill after writing or modifying code to verify correctness via unit tests,
  when diagnosing test failures, or when writing new test files.
  Triggers: "run tests", "test fails", "write a test", any gdUnit4/unit test mention.
  Supports both GDScript (.gd) and C# (.cs) test files.
---

# gdUnit4 Test Driver

$ARGUMENTS

## 1. Locate the Godot executable

Use `GODOT_PATH` when Godot is not on `PATH`:

```bash
GODOT_BIN="${GODOT_PATH:-godot}"
"$GODOT_BIN" --version
```

## 2. Run tests

The CLI runner across all supported versions (v4.x, v5.x, v6.x) is `addons/gdUnit4/bin/GdUnitCmdTool.gd`. The path uses capital-U `gdUnit4/` to match the upstream repo layout — Windows is case-insensitive but Godot's global script registry de-duplicates by exact path string, so a casing mismatch between the runner invocation and the on-disk directory triggers `Class "..." hides a global script class` parse errors and a non-zero exit.

```bash
# Single file
"$GODOT_BIN" --headless -s res://addons/gdUnit4/bin/GdUnitCmdTool.gd \
  --add res://test/test_example.gd --ignoreHeadlessMode

# Multiple files
"$GODOT_BIN" --headless -s res://addons/gdUnit4/bin/GdUnitCmdTool.gd \
  --add res://test/test_physics.gd --add res://test/test_spawner.gd \
  --ignoreHeadlessMode

# All tests in a directory
"<godot_path>" --headless -s res://addons/gdUnit4/bin/GdUnitCmdTool.gd \
  --add res://test/ --ignoreHeadlessMode
```

Notes:
- Include `--ignoreHeadlessMode` for headless runs.
- Use `--add` to enqueue test files or directories (repeat the flag for multiples).
- Runner path needs the `res://` prefix and the capital-U `addons/gdUnit4/` casing.
- No `::method` syntax for single test methods — run the whole file instead.

### C# tests

`GdUnitCmdTool.gd` supports C# test files too, but ensure `dotnet build` passes first — gdUnit4 runs compiled assemblies, not source files.

```bash
dotnet build && "$GODOT_BIN" --headless -s res://addons/gdUnit4/bin/GdUnitCmdTool.gd \
  --add res://test/csharp/TestExample.cs --ignoreHeadlessMode
```

### Useful flags

| Flag | Purpose |
|------|---------|
| `--add <path>` | Add test path to execution (file or directory; repeat to enqueue multiple) |
| `--ignoreHeadlessMode` | Allow headless execution |
| `--report-directory <path>` | Override report output directory |

### Timeout

gdUnit4 has a default test timeout (configurable in `GdUnitSettings`). If tests hang:
- Check for infinite loops or unresolved `await` calls
- Tests involving scene tree operations need explicit timeouts on await calls
- Kill the process after 120 seconds if no output appears

## 3. Parse results

### Stdout parsing

`GdUnitCmdTool.gd` output contains ANSI color codes — strip them before parsing. Format:

```
Run Test Suite res://test/test_example.gd
  Run Test: res://test/test_example.gd > test_basic_math :PASSED 38ms
  Run Test: res://test/test_example.gd > test_will_fail :FAILED 39ms
  Report:
    line <n/a>: Expecting:
     '2'
     but was
     '1'

Statistics: | 2 tests cases | 0 error | 1 failed | 0 flaky | 0 skipped | 0 orphans |
Executed test suites: (1/1)
Executed test cases: (2/2)
Total time:        128ms
Exit code: 100
```

Notes:
- Per-test lines start with `Run Test:`; suite lines with `Run Test Suite`.
- Duration is reported in milliseconds (`38ms`).
- Failure line numbers may show `line <n/a>` instead of exact lines.
- Summary uses `Statistics:` with pipe-delimited counts.
- Exit code 100 = test failures (not 1).

Extract from each test line:
- **Name**: the `test_*` function name (after `>`).
- **Status**: `PASSED`, `FAILED`, `SKIPPED`, `ERROR`.
- **Duration**: `Nms` after the status.
- **Failure message**: indented `Report:` lines following a FAILED test (assertion details + source location if available).

### JUnit XML report

Use `--report-directory <path>` for JUnit XML reports. The default report
directory is `res://reports/`, which maps to project-root `reports/`.

```xml
<testsuites>
  <testsuite name="TestExample" tests="4" failures="1" errors="0" skipped="1">
    <testcase name="test_basic_math" classname="TestExample" time="0.002"/>
    <testcase name="test_will_fail" classname="TestExample" time="0.003">
      <failure message="Expecting '2' but was '1'" type="AssertionError">
        at: res://test/test_example.gd:15
      </failure>
    </testcase>
  </testsuite>
</testsuites>
```

### Structured output format

Report results in this format:

```
## Test Results: test_example.gd

| Test | Status | Duration |
|------|--------|----------|
| test_basic_math | PASS | 0.002s |
| test_string_concat | PASS | 0.001s |
| test_will_fail | FAIL | 0.003s |
| test_skipped | SKIP | 0.000s |

**Summary: 4 total, 2 passed, 1 failed, 1 skipped, 0 errors**

### Failures

**test_will_fail** (res://test/test_example.gd:15)
> Expecting '2' but was '1'
```

Always include the file:line for failures — the agent (or user) needs this to navigate to the problem.

## 4. Writing tests

When the agent needs to write a new test file, follow these patterns.

### GDScript test structure

```gdscript
# res://test/test_my_system.gd
extends GdUnitTestSuite

# Runs before each test
func before_test() -> void:
    pass

# Runs after each test
func after_test() -> void:
    pass

func test_example() -> void:
    assert_int(2 + 2).is_equal(4)

func test_string_operations() -> void:
    assert_str("hello").contains("ell")
```

### Key assertion API

```gdscript
# Integers
assert_int(value).is_equal(expected)
assert_int(value).is_greater(threshold)
assert_int(value).is_between(low, high)

# Floats
assert_float(value).is_equal_approx(expected, 0.001)

# Strings
assert_str(value).is_equal(expected)
assert_str(value).contains(substring)
assert_str(value).starts_with(prefix)

# Booleans
assert_bool(value).is_true()
assert_bool(value).is_false()

# Objects
assert_that(value).is_not_null()
assert_that(value).is_instanceof(MyClass)

# Arrays
assert_array(arr).has_size(3)
assert_array(arr).contains([1, 2])

# Signals
await assert_signal(node).is_emitted("my_signal")
await assert_signal(node).wait_until(2.0).is_emitted("my_signal")

# Errors / Warnings (assert that code pushes expected error)
assert_error(callable).is_push_error("expected message")
```

### Scene fixtures

When testing code that needs nodes or scene tree:

```gdscript
func test_with_scene() -> void:
    # auto_free ensures cleanup even if test fails — prevents scene leaks
    var scene = auto_free(load("res://scenes/player.tscn").instantiate())
    add_child(scene)

    # Now test with the live scene
    assert_that(scene.get_node("Sprite2D")).is_not_null()
```

`auto_free()` is critical — without it, test failures leak nodes and eventually crash the runner.

### Stub design

A stub class must expose every property and method the system-under-test reads or calls on it.

```gdscript
# WRONG — bare Node has no global_position, no velocity
var entity = auto_free(Node.new())
system.process_one(entity)  # Invalid access to property "global_position"

# RIGHT — stub class carries the properties the system reads
var entity = auto_free(CharacterBody2D.new())
system.process_one(entity)
```

Grep the system code for every property and method it touches on the stubbed argument; each one must exist on the stub class.

### Async tests

For code involving signals, timers, or physics frames:

```gdscript
func test_signal_emission() -> void:
    var emitter = auto_free(SignalEmitter.new())
    add_child(emitter)

    emitter.trigger_action()
    await assert_signal(emitter).wait_until(2.0).is_emitted("action_done")

func test_after_physics_frame() -> void:
    var node = auto_free(MyNode.new())
    add_child(node)

    # Wait for physics to process
    await get_tree().physics_frame
    assert_float(node.position.x).is_greater(0.0)
```

Always pass a timeout to `wait_until()` — unbounded waits hang the test runner.

### SceneRunner (frame and input simulation)

When tests need `_process` / `_physics_process` to actually execute, or need to simulate player input, use gdUnit4's SceneRunner. Without it, adding a node to the tree does NOT advance frames — process callbacks never fire.

**GDScript:**

```gdscript
func test_player_moves_on_input() -> void:
    var runner := scene_runner("res://scenes/player.tscn")

    # Simulate input action (matches Input Map)
    runner.simulate_action_pressed("move_right")
    await runner.simulate_frames(5)

    var player = runner.scene()
    assert_float(player.position.x).is_greater(0.0)

func test_physics_movement() -> void:
    var runner := scene_runner("res://scenes/player.tscn")
    runner.set_property("velocity", Vector2(100, 0))

    # Advance 10 frames — _physics_process runs each frame
    await runner.simulate_frames(10)

    assert_float(runner.scene().position.x).is_greater(0.0)
```

**C#:**

```csharp
[TestCase]
public async Task TestPlayerMovesOnInput()
{
    var runner = ISceneRunner.Load("res://scenes/Player.tscn");

    runner.SimulateActionPressed("move_right");
    await runner.SimulateFrames(5);

    var player = runner.Scene<PlayerController>();
    AssertFloat(player.Position.X).IsGreater(0.0f);
}
```

**Key SceneRunner methods:**

| GDScript | C# | Purpose |
|----------|-----|---------|
| `scene_runner(path)` | `ISceneRunner.Load(path)` | Load scene for testing |
| `runner.scene()` | `runner.Scene()` | Get the root node |
| `await runner.simulate_frames(n)` | `await runner.SimulateFrames(n)` | Advance N frames |
| `runner.set_property(name, val)` | `runner.SetProperty(name, val)` | Set node property |
| `runner.simulate_key_press(key)` | `runner.SimulateKeyPress(key)` | Simulate key press |
| `runner.simulate_action_pressed(action)` | `runner.SimulateActionPressed(action)` | Simulate Input Map action |
| `runner.simulate_mouse_move_absolute(pos)` | `runner.SimulateMouseMoveAbsolute(pos)` | Move mouse to position |
| `await runner.await_input_processed()` | `await runner.AwaitInputProcessed()` | Wait for input processing |

**When to use SceneRunner vs plain auto_free:**
- **Pure logic** (math, data transforms): plain `auto_free(Node.new())` + direct calls — faster, simpler
- **Needs process callbacks**: SceneRunner — process/physics actually run
- **Needs input simulation**: SceneRunner — only way to simulate keys/mouse/actions in tests

### C# test structure

```csharp
// res://test/csharp/TestMySystem.cs
using GdUnit4;
using static GdUnit4.Assertions;

[TestSuite]
public partial class TestMySystem : TestSuite
{
    [TestCase]
    public void TestExample()
    {
        AssertInt(2 + 2).IsEqual(4);
    }

    [TestCase]
    public async Task TestAsync()
    {
        var node = AutoFree(new MyNode());
        AddChild(node);
        await AssertSignal(node).WaitUntil(2000).IsEmitted("ready");
    }
}
```

## 5. Common issues

### "No tests found"
- Test file must `extends GdUnitTestSuite`
- Test methods must start with `test_` (GDScript) or have `[TestCase]` attribute (C#)
- File path must be correct — use `res://` paths, not absolute OS paths

### Test hangs indefinitely
- Missing timeout on `await` calls — always use `wait_until(seconds)` / `WaitUntil(ms)`
- Process callbacks never fire — use SceneRunner with `simulate_frames()` instead of raw `add_child()`
- Infinite loop in the code under test
- Scene tree waiting for a signal that never fires
- Kill the process and check which test was last reported

### Scene leaks / orphan nodes
- Always wrap instantiated nodes with `auto_free()`
- If `before_test()` creates nodes, `after_test()` should clean them up
- gdUnit4 reports orphan nodes at the end — treat these as test failures

### "Cannot load script" errors
- Check `class_name` conflicts between test and production code
- Ensure the test file has valid syntax (run headless-build first)
- For C#: run `dotnet build` before running tests

### "Headless mode is not supported" (exit 103)
- `GdUnitCmdTool` refuses headless by default
- Add `--ignoreHeadlessMode` to the command line — this is safe for non-UI tests
- Input simulation tests won't work in headless (InputEvents are not transported)

### "Cannot load script" for the runner itself
- Path must start with `res://` and use capital-U casing: `res://addons/gdUnit4/bin/GdUnitCmdTool.gd`.
- On case-sensitive filesystems (Linux/macOS), a directory named `addons/gdunit4/` will not resolve at all; on Windows it resolves but Godot's class registry will double-register classes under the two casings and emit `Class "..." hides a global script class` parse errors.
- The runner filename is `GdUnitCmdTool.gd` for every supported version (v4.x / v5.x / v6.x).

### CSharpScript errors on mono builds
- `Nonexistent function 'new' in base 'CSharpScript'` — this happens when gdUnit4 tries to load C# support but the C# assembly isn't built. Run `dotnet build` first, or ignore these errors if you only run GDScript tests.

### Exit code interpretation
- **Exit 0**: all tests passed
- **Exit 100**: one or more tests failed
- **Exit 103**: headless mode refused — add `--ignoreHeadlessMode`
- **Exit 101 / others**: runner itself crashed or Godot bailed before tests ran (check stderr)
