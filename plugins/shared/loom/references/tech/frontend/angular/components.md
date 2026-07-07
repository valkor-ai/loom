# Angular Component Quality

This file applies Angular component-level discipline to task-owned standalone components, templates, input/output contracts, content projection, and reusable UI boundaries.

## When To Use

- The task creates or changes Angular components, shared UI primitives, feature components, forms, tables, detail panels, modals, or component composition.
- Use this when component API shape, template control flow, projection slots, change detection, accessibility, or parent-child data flow affects the delivered workflow.
- Keep broader state-store decisions in the NgRx reference and broader stream decisions in the RxJS reference.

## Implementation Focus

- Design component APIs around product concepts, not backend transport shapes. Editable forms, filters, selected rows, and confirmation dialogs often need view models distinct from API DTOs.
- Use required inputs only when the component cannot render a meaningful fallback. Optional inputs need explicit default rendering, disabled state, or empty state behavior.
- Emit events with enough context for the parent to act safely. For row actions, emit stable identifiers or snapshots that cannot drift if filters or selection change before the handler runs.
- Keep two-way `model<T>()` bindings for simple controlled values. Use explicit input/output pairs for complex workflows where saving, validation, or confirmation has separate states.
- Keep presentational components free of direct HTTP calls, router decisions, store dispatches, and hidden business mutations. Those belong in a container, facade, service, or store boundary.
- Use content projection only for real extension points such as card header/body/footer, toolbar actions, table empty state, or modal footer. Do not use projection to hide unrelated page composition in a generic shell.
- Keep templates readable. Move repeated conditions, labels, and eligibility rules into named computed values or methods with no side effects.
- Use semantic controls and accessible names. Icon-only buttons, menu triggers, custom selects, dialogs, and destructive actions must expose labels and focus behavior.
- Keep styles aligned with the repository's design system or UIX tokens. Do not add one-off inline styling when the app has shared component classes, tokens, or theme primitives.
- Avoid mixing old and new Angular syntax in the same new component unless compatibility requires it. A new standalone component should not introduce NgModule declarations.

## Verification Focus

- Test component rendering through public inputs, visible output, and user events rather than private fields.
- Verify loading, empty, populated, validation error, blocking error, disabled, and destructive confirmation states owned by the component.
- Verify emitted events target the displayed record after sorting, filtering, pagination, modal open/close, or repeated list refresh.
- Verify focus handling for dialogs, drawers, menus, and error summaries when the component introduces an overlay or custom interaction.
- Run component tests with TestBed or the repository's existing Angular testing stack, plus build/lint when component templates or imports changed.

## Evidence Focus

- In the evidence summary, name the component decision: required input, event payload contract, signal/model usage, projection slot, OnPush boundary, accessible control, or action-targeting proof.
