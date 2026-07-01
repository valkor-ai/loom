# UIX Focus: Content

Load this when visible product copy, labels, empty states, validation, status, or help text are created or changed.

## Product Copy Rules

- Use the user's product language and locale.
- Write labels around user tasks and business objects, not implementation structures.
- Avoid Loom/MCP/internal workflow terms in product UI.
- Avoid explaining runtime commands, verification steps, or technical stack unless the product is a developer tool and the user task requires it.
- Keep page headings concrete: name the product area or business task.

## State Copy

- Loading: short and scoped to the region.
- Empty: explain the business state and next action.
- Validation: tell the user what to fix and where.
- Error: explain recovery, not stack traces.
- Business-blocking: state the domain rule and affected object.
- Success: confirm what changed and show the new status when useful.

## Controls

- Button labels should be verbs or clear commands.
- Destructive labels should name the destructive action.
- Icon-only controls require accessible labels.
- Avoid generic labels such as "Submit" when the business action is known.

## Dense UI

- Tables and badges need short, consistent status labels.
- Tooltips should clarify controls, not contain critical information that mobile users cannot access.
- Long help text belongs in a side panel, detail section, or docs route rather than inside every row.
