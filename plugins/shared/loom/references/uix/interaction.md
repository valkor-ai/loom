# UIX Focus: Interaction

Load this when the task creates or changes user actions, forms, navigation, tables, modals, drawers, command surfaces, or feedback behavior.

## Action Design

- Primary action is visible where the user makes the decision.
- Secondary actions are available but visually quieter.
- Destructive actions need confirmation, undo, or a clear recovery path based on severity.
- Disabled actions should explain why when the user can do something to unlock them.
- Repeated row actions should keep row identity stable and visible.
- Long-running actions show progress at the action source. Keep the object identity visible while the request is pending.
- Mutations should define what changes after success: row state, detail summary, event history, count, or navigation.

## Forms

- Use visible labels, not placeholder-only labels.
- Group related fields and explain business requirements near the field.
- Validate before submit when rules are known locally.
- Preserve user input after validation or server failure.
- Show submitting state and prevent accidental double-submit.
- Place business-blocking feedback near the affected field/object and in the form summary when useful.

Form anatomy for business applications:

```html
<form data-region="business-form">
  <header data-region="form-context"></header>
  <section data-region="field-group"></section>
  <section data-region="validation-summary" aria-live="polite"></section>
  <section data-region="business-block" aria-live="polite"></section>
  <footer data-region="form-actions"></footer>
</form>
```

Do not collapse validation, technical error, and business block into one generic toast. They require different user recovery paths.

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

Use this placement guide:

| Feedback type | Preferred placement |
| --- | --- |
| Field validation | Directly under the field and in summary for long forms. |
| Business-blocking rule | Affected row/detail/action panel, with optional form summary. |
| System failure | Region-level alert with retry path. |
| Successful mutation | Updated object state plus short toast or inline confirmation. |
| Empty result | Results region, filters still visible. |

## State Region Pattern

```html
<form data-state="ready|submitting|blocked|error">
  <div data-region="field-errors"></div>
  <div data-region="business-block"></div>
  <div data-region="actions"></div>
</form>
```

Keep technical errors, validation errors, and business-rule blocks visually distinct. Users should know whether to retry, correct input, or change a business condition.

## Keyboard And Pointer

- Focus order follows task order.
- Icon-only controls have labels and tooltips when meaning is not universal.
- Touch targets are large enough and separated.
- Hover states must have focus/touch equivalents.
- Escape/back behavior should close transient layers before abandoning the whole workflow.

## Workflow Continuity

- Preserve selected rows, filters, active tabs, and entered values across refreshes and failed mutations when technically possible.
- Detail drawers should close back to the same list state.
- Multi-step flows need progress, back/cancel behavior, and a visible summary before irreversible actions.
- If a task spans frontend and backend, the UI must display backend validation and domain errors in product language.

## Quality Gate Index

| Gate | Pass signal | Fail signal |
| --- | --- | --- |
| `admin.state.scoped_feedback` | Loading, success, validation, error, and business-blocking feedback appear beside the affected table, form, detail, row, or action. | Feedback is only a toast/global banner, domain blocks look like technical errors, or failed submit loses context/input. |
