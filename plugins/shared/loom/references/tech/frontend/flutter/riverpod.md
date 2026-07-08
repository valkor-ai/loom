# Flutter Riverpod Quality

This file applies Riverpod discipline to task-owned providers, notifiers, async notifiers, provider scopes, provider families, selectors, and Riverpod-backed UI workflows.

## When To Use

- The task creates or changes Riverpod providers, generated `@riverpod` notifiers, `ConsumerWidget` screens, `WidgetRef` reads/watches, async state, provider families, or provider overrides in tests.
- Use this when Riverpod is the repository's selected state boundary or when the task explicitly names Riverpod, providers, notifiers, `WidgetRef`, or `ProviderScope`.
- Do not introduce Riverpod into a Bloc-only or plain Flutter codebase unless the technical baseline or existing repository already uses it.

## Implementation Focus

- Keep provider state immutable. Replace lists/maps/models with new instances; do not mutate existing state in place.
- Choose provider types by lifecycle and behavior: `Provider` for derived values/services, `StateProvider` for simple local mutable values, `FutureProvider` for read-only async loads, `StreamProvider` for streams, and Notifier/AsyncNotifier for command-bearing feature state.
- Use `ConsumerWidget`, `ConsumerStatefulWidget`, or scoped `Consumer` only where the widget needs provider access. Do not convert pure presentational widgets into consumers without a state reason.
- Use `ref.watch` for rendering dependencies and `ref.read` for event handlers/commands. Avoid `watch` inside callbacks.
- Use `select` to limit rebuilds when widgets need one field from a larger state object.
- Model async state with `AsyncValue` and explicit `data/loading/error` rendering. Keep business-blocking failures distinct from transport or unexpected failures.
- Keep provider families keyed by stable domain identifiers. Avoid passing mutable view objects as provider family arguments.
- Use provider overrides in tests for repositories, clocks, storage, API clients, and permissions. Do not hit real infrastructure from provider tests.
- Keep generated providers in sync when using Riverpod annotations and build-runner.

## Verification Focus

- Test provider state transitions for success, failure, loading, refresh, command outcomes, and immutability.
- Test Consumer widgets render each relevant `AsyncValue` state and dispatch commands through `ref.read`.
- Verify `select` or provider scoping prevents broad rebuilds when performance is part of the task.
- Verify provider overrides isolate tests from real API/storage/native services.
- Run `flutter analyze`, focused provider/widget tests, and code generation when annotations change.

## Evidence Focus

- In the evidence summary, name the Riverpod decision: provider type, notifier command, AsyncValue contract, provider family key, selector, override strategy, generated provider update, or provider-test proof.
