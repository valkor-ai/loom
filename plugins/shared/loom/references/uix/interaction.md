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

## Brief Mapping

When `uiProductionBrief.actionContract` is present:

- `primaryActions` must be directly reachable in the owning surface or flow.
- `contextualActions` stay attached to the relevant row, record, field group, panel, or step.
- `dangerousActions` require confirmation, undo, or a clear recovery path based on severity.
- `placementRule` overrides generic component habits; keep the affected object visible at decision time.
- `postSuccessUpdate` is part of the implementation, not just copy. Update the row, detail, count, state, route, or history that proves the mutation landed.

When `uiProductionBrief.stateContract` is present:

- Implement each listed state at the affected region, not only as a global spinner, banner, or toast.
- Keep validation, technical failure, domain blocking, and disabled/unavailable states visually and semantically distinct.
- For integration-backed actions, pending/error/success feedback must preserve object identity and user-entered values when practical.

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

## Interaction Composition

Choose the smallest interaction surface that preserves context:

| Situation | Preferred composition |
| --- | --- |
| Quick field edit | Inline edit with field validation and cancel/recovery. |
| Row or record inspection | Detail panel or route that preserves list context. |
| Short confirmation | Dialog with explicit consequence and focused primary action. |
| Multi-field or deep workflow | Dedicated route or step flow with back/cancel and summary. |
| Temporary supporting choice | Popover, menu, or sheet with keyboard and touch exit. |
| Long-running mutation | Source action stays visible with pending, retry, and readback state. |

Do not use a modal as the default container for every interaction. Choose a layer
from the amount of context, text, validation, and navigation the user needs.

## Transition And Reconciliation

- Define the allowed transition before styling the control: eligible, pending,
  succeeded, failed, blocked, and unavailable are distinct states.
- Disable only the action that cannot safely repeat. Keep navigation, cancellation,
  and unaffected work available when the product allows it.
- Reconcile optimistic UI with the returned server state. On failure, restore the
  prior value or show the server result and the recovery path.
- Keep selected identity, filters, draft input, and active tab stable across a
  retry or a route transition when those values define the user's context.
- Test rapid repeated input, double activation, escape/back, refresh during pending,
  and a failure after a visible optimistic update when the flow is asynchronous.

## Workflow Continuity

- Preserve selected rows, filters, active tabs, and entered values across refreshes and failed mutations when technically possible.
- Detail drawers should close back to the same list state.
- Multi-step flows need progress, back/cancel behavior, and a visible summary before irreversible actions.
- If a task spans frontend and backend, the UI must display backend validation and domain errors in product language.

## Quality Gate Index

| Gate | Pass signal | Fail signal |
| --- | --- | --- |
| `admin.state.scoped_feedback` | Loading, success, validation, error, and business-blocking feedback appear beside the affected table, form, detail, row, or action. | Feedback is only a toast/global banner, domain blocks look like technical errors, or failed submit loses context/input. |
