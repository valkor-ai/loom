# Angular Core Quality

This file applies Angular 17+ implementation discipline to task-owned application shells, screens, standalone components, services, and feature modules.

## When To Use

- The task creates or changes Angular application structure, standalone components, feature screens, forms, lists, services, or UI orchestration.
- Use this for Angular-specific component boundaries, signal usage, dependency injection, change detection, control flow syntax, and client-visible workflow behavior.
- If the task only changes route configuration, RxJS stream semantics, NgRx state, or tests, pair this file with the focused Angular reference for that topic.

## Implementation Focus

- Preserve the repository's Angular version, CLI conventions, folder layout, styling system, component library, and test runner before adding new patterns.
- Prefer standalone components for new Angular work unless the repository is intentionally NgModule-based and the task must remain compatible with that module boundary.
- Use `ChangeDetectionStrategy.OnPush` for task-owned components that render business data, lists, forms, or status panels. Keep inputs immutable enough for OnPush to be meaningful.
- Use signals for local UI state, derived labels, eligibility flags, and small view models. Use `computed()` for derived state rather than recalculating values in templates or event handlers.
- Use signal `input()`, `input.required()`, `output<T>()`, and `model<T>()` when the repository is already on a compatible Angular version. Keep older `@Input()` / `@Output()` style only when existing code or Angular version requires it.
- Keep container/smart components responsible for data loading, navigation decisions, and business actions. Keep presentational components focused on inputs, outputs, rendering, and accessibility.
- Use modern control flow (`@if`, `@for`, `@switch`) in new compatible code. Use stable domain identifiers in `@for ... track`; do not track mutable array indexes for dynamic rows.
- Use `inject()` consistently in new services, guards, resolvers, effects, and components when existing Angular style supports it. Do not create constructor injection churn in files that intentionally use constructors.
- Keep business-blocking responses explicit in the UI. Do not collapse permission denial, validation failure, duplicate record, stale state, and transport failure into the same generic message.
- Keep product UI free of delivery notes, runtime commands, framework explanations, verification instructions, and implementation progress text.

## Verification Focus

- Run the repository's Angular build, typecheck, lint, and focused component/service tests when available.
- Verify loading, empty, ready, validation error, business-blocking error, submitting, success, and disabled states touched by the task.
- For lists and tables, verify stable tracking, action targeting, sorting/filtering, empty state, and error state.
- For forms, verify editable draft separation from persisted records, field-level validation, submit disablement, backend error display, and resubmit behavior.
- For OnPush and signal work, verify the view updates after task-owned state changes without forcing unrelated refreshes or brittle manual change detection.

## Evidence Focus

- In the evidence summary, name the Angular decision: standalone boundary, signal state model, OnPush rendering, container/presentational split, control-flow tracking, DI style, or explicit business state handling.
