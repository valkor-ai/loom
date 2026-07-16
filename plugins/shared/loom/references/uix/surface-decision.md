# UIX Surface Decision

Load this file when a task includes a structured surface decision, custom UI
pattern work, or any page/screen whose product shape does not fit a single
obvious scenario.

This file explains how to turn a surface decision into implementation. It does not ask you to derive reference plans, quality rules, or schema fields. Those arrive in the task context. Your job is to make the rendered product surface match the selected decision.

## Decision Modes

Use the selected mode as an implementation strategy:

| Mode | Implementation meaning |
| --- | --- |
| `known` | Use the named pattern as the dominant structure. Keep scenario anatomy intact, then adapt labels, data, actions, and states to the product. |
| `hybrid` | Use the primary known pattern for layout ownership and combine only the declared secondary patterns for specific regions or interactions. |
| `custom` | Build a product-specific surface from semantic facts, nearest known patterns, layout model, regions, actions, states, and content boundary. Custom is stricter than known mode because nothing can be hand-waved to a canned scenario. |

Known patterns are not page templates. A `collection_workbench` can be implemented with React, Vue, native mobile, server-rendered HTML, or another stack. The pattern controls regions and workflow, not class names.

## Pattern Matching

Before editing visible UI, map the contract into a short working model:

```text
user job -> information shape -> operation model -> risk factors
layout model -> region ids -> actions -> states -> quality rules
content boundary -> copy tone -> forbidden visible content
nearest known patterns -> reusable anatomy -> custom differences
```

Choose the layout that supports the dominant job:

- Collection or queue work: list/table/card stream plus filters, selection, detail, and scoped actions.
- Form flow: grouped form, validation, review/submit feedback, and recovery path.
- Analytics monitor: time/range controls, chart/table pairing, anomaly/status explanation, and drill-down.
- Editor workspace: persistent canvas/editor, inspector, command/action region, save/version state.
- Support inbox: queue, conversation/detail, assignment/status, reply/action state.
- Developer console: technical object explorer, request/result surfaces, logs only when the product task needs them.
- Content or marketing surface: narrative hierarchy, proof, media, and conversion path rather than operational density.
- Mobile task flow: one primary task per screen, bottom/inline actions, touch-safe controls, and list-to-detail navigation.

## Custom Mode Rules

For `custom`, do all of the following:

- Name the nearest known patterns in your own implementation notes, then implement only the parts the task actually owns.
- Build every declared region. If a region is not visible in this task, record why it is out of scope instead of silently dropping it.
- Implement every task-owned action with pending, success, error, disabled, and business-blocking behavior when those states are in scope.
- Put state feedback near the affected region or control. A global toast is never enough for validation, loading, empty, or business-blocking states.
- Convert abstract information shapes into concrete labels, fields, summaries, tables, charts, cards, or detail panels.
- Keep visual style production-grade for the product mode. Do not use "custom" as permission for decorative experiments, generic demo cards, or unstructured page sections.
- Use semantic tokens or the existing design system before creating new styles.

## Region Implementation

Treat the declared task-owned regions as the visible work map.

```html
<main data-surface="task">
  <header data-region="context"></header>
  <section data-region="primary-work"></section>
  <aside data-region="detail-or-support"></aside>
  <section data-region="feedback"></section>
</main>
```

The names above are placeholders. Use the contract's region ids and the project's component style, but preserve these responsibilities:

- Context region: current object, section, search/filter context, user/workspace context when relevant.
- Primary work region: list, table, form, editor, chart, or task canvas where the main job happens.
- Support/detail region: selected record, explanation, history, preview, inspector, or secondary controls.
- Feedback region: scoped loading, validation, empty, error, success, disabled, and business-blocking states.

## Action Implementation

Actions must be placed where the user decides:

- Primary actions belong near the work region or sticky action bar, not in a decorative header far away.
- Row/detail actions stay attached to the row/detail they affect.
- Destructive actions need confirmation, undo, or a clear recovery path based on severity.
- Async actions need pending state, double-submit protection, success update, and recoverable error message.
- Disabled actions need a visible reason when the user can fix the block.

## State Implementation

Use the surface contract states as acceptance targets:

| State | Implementation proof |
| --- | --- |
| loading | Stable region skeleton/progress without layout jump. |
| empty | Business reason plus next valid action or explanation. |
| error | Recoverable message, retry/correction path, no stack trace. |
| validation | Field/control-level message, preserved input, focus path. |
| success | Updated affected object plus confirmation near the change. |
| disabled | Reason or eligibility hint, not silent opacity alone. |
| business_blocking | Product rule explanation tied to the affected action or object. |

## Evidence Mapping

When writing UI quality evidence:

- Region evidence names changed UI files and the region implemented.
- Action evidence names changed UI files and how the action behaves through pending/success/error/disabled states.
- State evidence names changed UI files and where each state is rendered.
- Quality-rule evidence names changed UI files or rendered checks that prove each quality rule.
- Content-boundary evidence states what product copy was checked and whether forbidden internal/process content appears.
- Read-reference evidence lists only the selected UIX files actually read for this task.

Evidence is not a prose compliment. It must point to concrete files, states, actions, or rendered checks.

## Quality Bar

A surface decision is satisfied only when:

- The implemented first visible surface is the product surface for the selected job.
- Layout regions are visible, useful, and responsive for the declared device posture.
- Information density matches the usage pattern: repeat work is compact; narrative surfaces are readable; immersive surfaces are intentionally framed.
- Actions and states are complete enough for a user to finish the workflow and recover from failure.
- Content stays inside the product boundary and avoids delivery/runtime/build/verification language.
- The design system or token source is reused or extended consistently.
