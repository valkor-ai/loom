# project.godot Configuration Reference

## Format

project.godot uses Godot's ConfigFile format (INI-like).
Sections: `[section_name]`, values: `key=value`.

## Sections Used by Scaffold

### [application]

```ini
config/name="Game Title"
run/main_scene="res://scenes/main.tscn"
config/features=PackedStringArray("4.4")
```

### [display]

```ini
window/size/viewport_width=1280
window/size/viewport_height=720
window/stretch/mode="canvas_items"
```

### [rendering]

```ini
renderer/rendering_method="gl_compatibility"
```

### [debug]

```ini
file_logging/enable_file_logging=false
```

Always include this section. It turns off Godot's default engine file log
(`user://logs/godot.log`, on the system drive), which has no per-file size cap
and can grow without bound during unattended pipeline / E2E runs where a
GDScript error is re-raised every frame. Diagnostics still reach the tooling
via stdout and the E2E log capture; verify redirects its own headless log with
an explicit `--log-file` (which overrides this setting).

### [physics]

```ini
# 2D gravity (platformers, endless runners)
2d/default_gravity=980.0

```

Only include this section for 2D games that use gravity.

## [input] — Action Mapping

Input actions use Godot's serialized Object format. Each action has a deadzone
and an events array containing input event objects.

### Single-key action format

```ini
action_name={
"deadzone": 0.2,
"events": [Object(InputEventKey,"resource_local_to_scene":false,"resource_name":"","device":-1,"window_id":0,"alt_pressed":false,"shift_pressed":false,"ctrl_pressed":false,"meta_pressed":false,"pressed":false,"keycode":0,"physical_keycode":KEYCODE,"key_label":0,"unicode":UNICODE)]
}
```

Replace `KEYCODE` and `UNICODE` from the table below.

### Multi-key action (e.g., A + Left Arrow)

Separate Object() entries with `, ` inside the events array:

```ini
move_left={
"deadzone": 0.2,
"events": [Object(InputEventKey,...,"physical_keycode":65,...,"unicode":97), Object(InputEventKey,...,"physical_keycode":4194319,...,"unicode":0)]
}
```

### Key Code Reference

| Key | physical_keycode | unicode |
|-----|-----------------|---------|
| A | 65 | 97 |
| B | 66 | 98 |
| D | 68 | 100 |
| E | 69 | 101 |
| Q | 81 | 113 |
| S | 83 | 115 |
| W | 87 | 119 |
| Space | 32 | 32 |
| Enter | 4194309 | 0 |
| Escape | 4194305 | 0 |
| Tab | 4194306 | 0 |
| Shift | 4194325 | 0 |
| Left Arrow | 4194319 | 0 |
| Right Arrow | 4194321 | 0 |
| Up Arrow | 4194320 | 0 |
| Down Arrow | 4194322 | 0 |

Arrow and special keys have `unicode=0`. Letter keys use their ASCII value.

### Full Input Event Template

Copy this template for each key, replacing KEYCODE and UNICODE:

```
Object(InputEventKey,"resource_local_to_scene":false,"resource_name":"","device":-1,"window_id":0,"alt_pressed":false,"shift_pressed":false,"ctrl_pressed":false,"meta_pressed":false,"pressed":false,"keycode":0,"physical_keycode":KEYCODE,"key_label":0,"unicode":UNICODE)
```
