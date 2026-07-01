# UIX Focus: Interaction

Load this when the task creates or changes user actions, forms, navigation, tables, modals, drawers, command surfaces, or feedback behavior.

## Action Design

- Primary action is visible where the user makes the decision.
- Secondary actions are available but visually quieter.
- Destructive actions need confirmation, undo, or a clear recovery path based on severity.
- Disabled actions should explain why when the user can do something to unlock them.
- Repeated row actions should keep row identity stable and visible.

## Forms

- Use visible labels, not placeholder-only labels.
- Group related fields and explain business requirements near the field.
- Validate before submit when rules are known locally.
- Preserve user input after validation or server failure.
- Show submitting state and prevent accidental double-submit.
- Place business-blocking feedback near the affected field/object and in the form summary when useful.

## Navigation

- Keep current section/page visible.
- Use breadcrumbs for deep management flows.
- Use tabs only for peer sections of the same object, not unrelated pages.
- Use drawers/sheets when they preserve list context; use full routes when the detail has deep workflow.
- Mobile navigation must not depend on hover.

## Feedback

- Success updates the affected object and gives a short confirmation.
- Error explains recovery without exposing stack traces or internal tool names.
- Loading is scoped to the region that is waiting.
- Toasts are for transient confirmation, not the only place for critical rules.

## Keyboard And Pointer

- Focus order follows task order.
- Icon-only controls have labels and tooltips when meaning is not universal.
- Touch targets are large enough and separated.
- Hover states must have focus/touch equivalents.
