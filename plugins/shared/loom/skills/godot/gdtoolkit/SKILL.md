---
name: gdtoolkit
description: |
  Lint and format GDScript files using gdtoolkit (gdlint + gdformat).
  Use after writing or modifying .gd files, when asked to check code style,
  fix lint errors, format code, or set up linting configuration.
  Also use when gdlint/gdformat errors appear in output and need diagnosis.
  Does NOT require Godot — runs as a standalone Python tool.
---

# gdtoolkit — GDScript Lint & Format

Use this as an optional local lint pass. Headless compilation and the project's
tests remain the acceptance checks; lint output alone is not a reason to change
working Godot code.

Wraps two CLI tools from the `gdtoolkit` Python package:
- **gdlint** — static analysis and style checking
- **gdformat** — deterministic auto-formatter

## Prerequisites

```bash
pip install "gdtoolkit==4.*"    # Godot 4 projects
pip install "gdtoolkit==3.*"    # Godot 3 projects
```

Verify installation: `gdlint --version && gdformat --version`

If not installed, tell the user and offer to install. The version major must match
the project's Godot major version (read from `project.godot` `config/features`).

## Lint workflow

### Run gdlint

```bash
gdlint path/to/file.gd          # single file
gdlint path/to/directory/       # all .gd files recursively
```

### Interpret output

Each problem is one line on stderr:
```
path/to/file.gd:42: Error: Function name "MyFunc" is not valid (function-name)
```

Format: `{file}:{line}: Error: {message} ({rule-id})`

Exit codes:
- **0** — no problems (stdout: `Success: no problems found`)
- **1** — problems found (stderr: `Failure: N problem(s) found`)

### Fix lint issues

For each reported issue, either:
1. **Fix the code** — rename to match convention, remove unused arg, reorder members
2. **Suppress inline** — when the violation is intentional:
   ```gdscript
   # gdlint:ignore = rule-id
   var _unusedButNeeded := 0   # this line + next line are suppressed
   ```
3. **Suppress region** — for larger blocks:
   ```gdscript
   # gdlint: disable=function-name
   func ALLCAPS_required_by_engine():
       pass
   # gdlint: enable=function-name
   ```

When suppressing, always add a brief comment explaining why.

## Format workflow

### Run gdformat

```bash
gdformat path/to/file.gd        # format in-place
gdformat path/to/directory/      # format all .gd files
gdformat --check path/           # check only, don't modify (exit 1 if changes needed)
gdformat --diff path/            # show unified diff on stderr, don't modify
```

Exit codes:
- **0** — already formatted / formatting succeeded
- **1** — check mode found differences / parse error / safety check failed

### Safety checks

gdformat runs three safety checks by default (disable with `--fast`):
- **TreeInvariantViolation** — parse tree changed after formatting
- **FormattingStabilityViolation** — formatting isn't idempotent
- **CommentPersistenceViolation** — comments were lost

If a safety check fails, report the error to the user — do NOT use `--fast` to bypass it.
This likely indicates a gdtoolkit bug; the file should be formatted manually or the
problematic section excluded.

## Rule reference

### Naming rules (regex-configurable)

| Rule ID | Default convention | Example |
|---|---|---|
| `function-name` | snake_case or `_on_PascalCase_signal` | `move_player`, `_on_Button_pressed` |
| `class-name` | PascalCase | `PlayerController` |
| `sub-class-name` | _PascalCase (leading underscore) | `_InternalHelper` |
| `signal-name` | snake_case | `health_changed` |
| `class-variable-name` | snake_case or _private | `speed`, `_cache` |
| `function-variable-name` | snake_case | `local_var` |
| `function-argument-name` | snake_case or _unused | `target_pos`, `_ignored` |
| `loop-variable-name` | snake_case or _unused | `item`, `_i` |
| `constant-name` | UPPER_SNAKE_CASE | `MAX_SPEED` |
| `enum-name` | PascalCase | `Direction` |
| `enum-element-name` | UPPER_SNAKE_CASE | `NORTH`, `SOUTH_EAST` |

### Code quality rules

| Rule ID | Default | What it checks |
|---|---|---|
| `max-returns` | 6 | Too many return statements per function |
| `max-public-methods` | 20 | Too many public methods per class |
| `function-arguments-number` | 10 | Too many function arguments |
| `max-file-lines` | 1000 | File too long |
| `max-line-length` | 100 | Line too long |

### Style rules

| Rule ID | What it checks |
|---|---|
| `unnecessary-pass` | `pass` in non-empty body |
| `duplicated-load` | Same resource loaded twice |
| `expression-not-assigned` | Standalone expression with no effect |
| `unused-argument` | Argument never used (fix: prefix with `_`) |
| `comparison-with-itself` | `x == x` |
| `private-method-call` | Calling `_private_method()` from outside |
| `class-definitions-order` | Members not in canonical order (see below) |
| `trailing-whitespace` | Trailing spaces |
| `mixed-tabs-and-spaces` | Mixed indentation |
| `no-elif-return` | Unnecessary elif after return |
| `no-else-return` | Unnecessary else after return |

### Class member order (class-definitions-order)

gdlint expects this top-to-bottom order:
1. `@tool`
2. `class_name`
3. `extends`
4. Docstring
5. Signals
6. Enums
7. Constants
8. Static variables
9. `@export` variables
10. Public variables
11. Private variables (`_prefixed`)
12. `@onready` public variables
13. `@onready` private variables
14. Remaining declarations

## Configuration

### gdlintrc

Create `.gdlintrc` (or `gdlintrc`) in the project root. YAML format.
gdlint searches upward from CWD, uses the first file found.

Generate defaults: `gdlint -d > .gdlintrc`

Example with customizations:
```yaml
# Relax line length to match gdformat default
max-line-length: 120

# Allow _on_NodeName_signal pattern (already default)
function-name: '(_on_[A-Z][a-z0-9]*(_[a-z0-9]+)*|[a-z][a-z0-9]*(_[a-z0-9]+)*)'

# Disable rules that conflict with project style
disable:
  - unnecessary-pass        # we use pass as explicit "intentionally empty"

# Exclude generated/vendored code
excluded_directories: !!set
  .git: null
  addons: null
  .godot: null
```

### gdformatrc

Create `gdformatrc` in the project root. YAML format.

Generate defaults: `gdformat --dump-default-config > gdformatrc`

```yaml
line_length: 120
# use_spaces: 4            # uncomment to use spaces instead of tabs
excluded_directories: !!set
  .git: null
  addons: null
  .godot: null
```

### Important: no pyproject.toml support

gdtoolkit does NOT read from `pyproject.toml`. Only its own YAML config files work.

### Recommended: align line lengths

Set `max-line-length` in `.gdlintrc` and `line_length` in `gdformatrc` to the same
value. If they differ, gdformat may merge lines that then exceed gdlint's limit —
a known source of false positives.

## Quirks and limitations

1. **No `# gdformat: off/on`** — there is no way to skip formatting for a code region.
   If gdformat mangles a specific construct, the only workaround is to restructure the code.

2. **gdlint checks unused arguments, not unused variables** — `var x = 1` with no
   further use of `x` will NOT be flagged. Only function arguments trigger `unused-argument`.

3. **`excluded_directories` only works when scanning directories** — passing a file path
   directly (`gdlint addons/plugin/main.gd`) bypasses exclusion rules.

4. **gdformat safety check failures are real bugs** — do not silence them with `--fast`.
   Report to the user and format that section manually.

5. **gdformat may cause data loss** — always ensure the file is under version control
   before formatting. Run `git diff` after formatting to verify changes are correct.

6. **Version must match Godot version** — gdtoolkit 4.x parses Godot 4 syntax,
   3.x parses Godot 3. Mismatched versions cause parse errors on valid code.

## Fallback: upstream documentation

If this skill's instructions don't resolve your issue — unexpected output, unfamiliar
rule IDs, config syntax errors, or parse failures — consult the upstream repo directly:

- **Repository**: https://github.com/Scony/godot-gdscript-toolkit
- **Linter wiki**: https://github.com/Scony/godot-gdscript-toolkit/wiki/3.-Linter
- **Formatter wiki**: https://github.com/Scony/godot-gdscript-toolkit/wiki/4.-Formatter
- **Open issues**: https://github.com/Scony/godot-gdscript-toolkit/issues

Use `WebFetch` to read the wiki pages or issue threads when you need details beyond
what this skill covers.
