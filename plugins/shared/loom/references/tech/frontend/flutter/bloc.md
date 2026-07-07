# Flutter Bloc Quality

This file applies Bloc/Cubit discipline to task-owned blocs, cubits, immutable states, events, listeners, providers, and event-driven Flutter workflows.

## When To Use

- The task creates or changes Bloc/Cubit state management, event-to-state transitions, feature workflow state, auth/forms/wizards, BlocProvider boundaries, BlocBuilder/Listener/Consumer usage, or bloc tests.
- Use Bloc when the repository already uses it or when the task explicitly requires event-driven state with predictable transitions.
- Do not introduce Bloc for tiny local widget state or into a Riverpod-only codebase unless the accepted architecture calls for it.

## Implementation Focus

- Keep events focused on user/system inputs and states focused on immutable UI workflow snapshots. Avoid vague events such as `UpdateEverything`.
- Use Cubit for simple command/state workflows and Bloc for event-driven workflows that need explicit event semantics, concurrency, or auditability.
- Keep states immutable with value equality, copy methods, Freezed, Equatable, or the repository's existing pattern.
- Keep business logic inside blocs/cubits or use cases, not inside widgets. Keep UI-only formatting and layout outside blocs.
- Use `BlocBuilder` for rendering, `BlocListener` for side effects, and `BlocConsumer` only when a widget truly owns both.
- Use `buildWhen` and `listenWhen` to avoid unnecessary rebuilds or duplicate side effects when state objects carry multiple fields.
- Do not use `context.watch` inside callbacks. Dispatch events or call cubit commands with `context.read`.
- Keep bloc provisioning at the narrowest stable boundary: route, feature shell, tab, or app root according to state lifetime.
- Keep navigation and snack/dialog side effects in listeners or route layers; blocs should emit intent/state, not hold `BuildContext`.

## Verification Focus

- Test blocs/cubits with explicit initial state, event/command, expected emitted states, and failure paths.
- Verify form, auth, wizard, mutation, and destructive flows emit disabled/submitting/success/failure states in the right order.
- Verify listeners fire once for side effects such as navigation, snack bars, dialogs, or analytics.
- Verify widgets dispatch the right event for the displayed record and render each relevant state.
- Run `flutter analyze`, bloc tests, and widget tests around BlocProvider/Builder/Listener integration.

## Evidence Focus

- In the evidence summary, name the Bloc decision: Cubit-vs-Bloc boundary, event contract, immutable state model, provider lifetime, buildWhen/listenWhen, side-effect listener, or bloc-test proof.
