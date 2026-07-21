# Input Mapper — Serialization Reference

## Object() Serialization Format

Each action in `[input]` uses this structure:

```ini
action_name={
"deadzone": 0.2,
"events": [EVENT_OBJECT, EVENT_OBJECT, ...]
}
```

### Keyboard Key Template

Copy this for each key, replacing `KEYCODE` and `UNICODE`:

```
Object(InputEventKey,"resource_local_to_scene":false,"resource_name":"","device":-1,"window_id":0,"alt_pressed":false,"shift_pressed":false,"ctrl_pressed":false,"meta_pressed":false,"pressed":false,"keycode":0,"physical_keycode":KEYCODE,"key_label":0,"unicode":UNICODE)
```

### Mouse Button Template

Replace `BUTTON_INDEX` (1=left, 2=right, 3=middle, 4=wheel_up, 5=wheel_down):

```
Object(InputEventMouseButton,"resource_local_to_scene":false,"resource_name":"","device":-1,"window_id":0,"alt_pressed":false,"shift_pressed":false,"ctrl_pressed":false,"meta_pressed":false,"button_mask":0,"position":Vector2(0, 0),"global_position":Vector2(0, 0),"factor":1.0,"button_index":BUTTON_INDEX,"canceled":false,"pressed":false,"double_click":false)
```

### Gamepad Button Template

Replace `BUTTON_INDEX` (0=A/Cross, 1=B/Circle, 2=X/Square, 3=Y/Triangle,
6=L1, 7=R1, 10=L3, 11=R3, 12=DPad_Up, 13=DPad_Down, 14=DPad_Left, 15=DPad_Right):

```
Object(InputEventJoypadButton,"resource_local_to_scene":false,"resource_name":"","device":-1,"button_index":BUTTON_INDEX,"pressure":0.0,"pressed":false)
```

### Gamepad Axis Template

Replace `AXIS` (0=LeftX, 1=LeftY, 2=RightX, 3=RightY, 6=TriggerL, 7=TriggerR)
and `AXIS_VALUE` (-1.0 or 1.0 for direction):

```
Object(InputEventJoypadMotion,"resource_local_to_scene":false,"resource_name":"","device":-1,"axis":AXIS,"axis_value":AXIS_VALUE)
```

### Multi-key Example

Multiple events separated by `, ` inside the array:

```ini
move_left={
"deadzone": 0.2,
"events": [Object(InputEventKey,"resource_local_to_scene":false,"resource_name":"","device":-1,"window_id":0,"alt_pressed":false,"shift_pressed":false,"ctrl_pressed":false,"meta_pressed":false,"pressed":false,"keycode":0,"physical_keycode":65,"key_label":0,"unicode":97), Object(InputEventKey,"resource_local_to_scene":false,"resource_name":"","device":-1,"window_id":0,"alt_pressed":false,"shift_pressed":false,"ctrl_pressed":false,"meta_pressed":false,"pressed":false,"keycode":0,"physical_keycode":4194319,"key_label":0,"unicode":0)]
}
```

## Key Code Reference

| Key | physical_keycode | unicode | Notes |
|-----|-----------------|---------|-------|
| A | 65 | 97 | |
| B | 66 | 98 | |
| C | 67 | 99 | |
| D | 68 | 100 | |
| E | 69 | 101 | |
| F | 70 | 102 | |
| G | 71 | 103 | |
| H | 72 | 104 | |
| I | 73 | 105 | |
| J | 74 | 106 | |
| K | 75 | 107 | |
| L | 76 | 108 | |
| M | 77 | 109 | |
| N | 78 | 110 | |
| O | 79 | 111 | |
| P | 80 | 112 | |
| Q | 81 | 113 | |
| R | 82 | 114 | |
| S | 83 | 115 | |
| T | 84 | 116 | |
| U | 85 | 117 | |
| V | 86 | 118 | |
| W | 87 | 119 | |
| X | 88 | 120 | |
| Y | 89 | 121 | |
| Z | 90 | 122 | |
| 0 | 48 | 48 | |
| 1 | 49 | 49 | |
| 2 | 50 | 50 | |
| 3 | 51 | 51 | |
| 4 | 52 | 52 | |
| 5 | 53 | 53 | |
| 6 | 54 | 54 | |
| 7 | 55 | 55 | |
| 8 | 56 | 56 | |
| 9 | 57 | 57 | |
| Space | 32 | 32 | |
| Enter | 4194309 | 0 | Special key |
| Escape | 4194305 | 0 | Special key |
| Tab | 4194306 | 0 | Special key |
| Backspace | 4194308 | 0 | Special key |
| Shift | 4194325 | 0 | Special key |
| Ctrl | 4194326 | 0 | Special key |
| Alt | 4194328 | 0 | Special key |
| Left Arrow | 4194319 | 0 | Special key |
| Right Arrow | 4194321 | 0 | Special key |
| Up Arrow | 4194320 | 0 | Special key |
| Down Arrow | 4194322 | 0 | Special key |
| F1 | 4194332 | 0 | F1-F12: 4194332-4194343 |
| F2 | 4194333 | 0 | |
| F3 | 4194334 | 0 | |

Letter keys: `physical_keycode` = uppercase ASCII, `unicode` = lowercase ASCII.
Number keys: both values equal the ASCII code.
Special/arrow keys: `unicode` is always `0`.
