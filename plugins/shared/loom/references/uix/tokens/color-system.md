# Color System Tokens

Use when adding or reviewing color decisions.

## Rules

- Prefer semantic roles over raw colors: surface, surface-muted, text, text-muted, border, action-primary, action-danger, success, warning, error, info, focus.
- Keep neutral colors slightly aligned to the product hue when creating a new system, but prioritize contrast and readability.
- Do not use color alone to express state. Pair color with text, icon, shape, or position.
- Avoid one-off color sprawl inside components. If the same role appears more than once, it should be tokenized or reused from the existing system.

## Evidence

TaskResult evidence should name the token source or existing system reused, and call out any hard-coded color kept for a concrete reason.
