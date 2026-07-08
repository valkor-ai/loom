# Flutter Widget Quality

This file applies Flutter widget discipline to task-owned screens, reusable widgets, forms, responsive layouts, slivers, dialogs, and composition boundaries.

## When To Use

- The task creates or changes Flutter widgets, layout composition, forms, lists, cards, dialogs, responsive surfaces, slivers, platform-specific widgets, or reusable UI components.
- Use this when widget APIs, build cost, keys, accessibility semantics, theming, or parent-child data flow affects the delivered workflow.
- Keep route decisions in the navigation reference and state-management decisions in Riverpod or Bloc references.

## Implementation Focus

- Give widgets typed constructor contracts that match product usage. Required constructor fields should be required in Dart; optional fields need deliberate defaults or fallback rendering.
- Use `StatelessWidget` for pure rendering and small local state only when state truly belongs to the widget. Do not use `setState` for app-wide, shared, persisted, or route-level state.
- Keep `build()` pure and cheap. Do not create controllers, futures, streams, providers, expensive lists, or service clients inside `build()`.
- Add `const` constructors and static children wherever possible. Static icons, labels, padding, gaps, and decoration should not be rebuilt as new objects unnecessarily.
- Use keys for dynamic child identity: filtered lists, repeated form fields, animated children, reorderable rows, tab pages, and preserved scroll positions.
- Use `LayoutBuilder`, `MediaQuery`, repository breakpoints, or responsive layout helpers for adaptive UI. Do not hardcode one phone size or desktop width as the layout contract.
- Use lazy builders for dynamic lists: `ListView.builder`, `GridView.builder`, slivers, or repository abstractions. Avoid building large mapped child arrays eagerly.
- Use `Semantics`, accessible labels, focus behavior, and proper tappable controls for custom widgets, icon-only actions, destructive actions, and dialogs.
- Keep theme usage centralized. Prefer `Theme.of(context)`, repository tokens, and existing component styles over one-off colors and text styles.

## Verification Focus

- Test widget behavior through visible text, semantics, input events, tap actions, validation messages, and parent callbacks.
- Verify loading, empty, populated, validation error, business-blocking error, disabled, focus, keyboard, and dialog states touched by the widget.
- Verify list identity and row action targeting after filtering, sorting, refresh, and navigation return.
- Verify responsive layout at representative mobile, tablet, desktop, and constrained widths when the task changes layout.
- Run widget tests and `flutter analyze` for changed widgets; use golden tests only when the repository already maintains them or visual regression risk is high.

## Evidence Focus

- In the evidence summary, name the widget decision: constructor contract, stateless/stateful boundary, const optimization, key strategy, lazy list, responsive layout, semantic control, or widget-test proof.
