# Flutter Application Implementation

Implement the accepted frontend experience within the repository's Flutter/Dart version, target platforms, theme/design system, state/navigation choices, API contract, and platform configuration. Do not replace established libraries because an external example prefers another stack.

## Application Boundary

Keep `main()` focused on required binding/config initialization and root app/provider scope. Put product workflows in feature screens/state/use cases and infrastructure behind repositories/services/adapters.

Widgets render state and emit intent. They should not construct API clients, persistence stores, permission adapters, or repositories inside `build`. Keep domain/business mutation out of reusable visual widgets.

Choose local `StatefulWidget` state for genuinely local ephemeral behavior; use the selected shared-state library only for cross-widget/route/business lifecycle. Riverpod and Bloc are alternatives unless the accepted architecture intentionally mixes bounded uses.

## Async Workflow State

Represent loading, empty, ready, validation, business-blocking, submitting, success, disabled, offline, stale, and unexpected failure states at the owning surface/control. Do not collapse all errors into one `Text('Error')` or treat failures as empty data.

Maintain a separate editable draft from persisted/server state. On save, preserve valid input, map field/global errors, block duplicates, reconcile returned identity/version/state, and make retry/recovery explicit.

Use mounted/context safety after awaits:

```dart
Future<void> submit() async {
  setState(() => submitting = true);
  final result = await repository.save(draft);
  if (!mounted) return;
  setState(() => submitting = false);
  result.fold(showFailure, applySavedRecord);
}
```

In shared-state code, prefer state/listener-driven side effects over direct `BuildContext` retention across async boundaries.

## Identity, Immutability, And Rebuilds

Use immutable state/models according to the repository pattern. Replace lists/maps/models rather than mutating watched state in place.

Use stable domain keys for filtered, sorted, pageable, animated, reorderable, or editable collections. Keys preserve element identity; they do not fix a state model keyed by array index.

Use `const` constructors/children where values are static and keep `build()` pure. Do not create controllers, futures, streams, focus nodes, providers, or clients during each build.

## Platform And Responsive Behavior

Follow the selected Material/Cupertino/custom design system consistently. Adapt navigation, safe areas, keyboard/insets, pointer/hover, text scaling, orientation, desktop window widths, and web URL behavior for task-owned target platforms.

Platform branches should be capability-driven and testable; do not scatter `Platform.is...` throughout widgets or import `dart:io` into web-compatible paths. Keep permissions and native plugins behind adapters with denied/permanently-denied/unavailable states.

Use responsive constraints/breakpoints based on content and repository rules. Dense business UI needs a usable narrow-screen composition, not a desktop table scaled down.

## API, Storage, And Security

Use typed repositories/services for accepted API paths, payloads, statuses, auth, pagination, and errors. Keep browser/mobile base URL and runtime binding consistent with deployment/platform networking; do not hardcode emulator/localhost addresses into product code.

Choose secure storage, preferences, database, or cache by data sensitivity/lifetime. Client storage is not secret against a compromised device/browser. Never embed service credentials or treat hidden UI/routes as authorization.

Handle connectivity as a signal, not proof a request will work. Preserve offline/stale policy and pending-write behavior only when accepted.

## Localization, Theme, And Content

Use the repository's localization generation and locale-aware dates/numbers/currency. Do not concatenate sentence fragments that cannot be translated or hardcode one locale's formatting.

Use theme extensions/tokens/components instead of one-off colors, spacing, text styles, and rounded cards. Respect text scaling, contrast, reduced motion, and long content.

Product UI must not expose runtime commands, framework notes, delivery progress, verification instructions, stack explanations, or debug errors.

## Verification

- Run focused analysis/tests for changed Dart and generated code.
- Exercise task-owned async states, draft/save/readback, duplicate blocking, and recovery.
- Verify stable row/action identity after filter/sort/page/refresh/navigation.
- Test target-platform keyboard/safe-area/permission/storage behavior when changed.
- Verify responsive, text-scale, localization, long-content, and semantics behavior for changed surfaces.
- Build/run the relevant target when plugin/platform configuration changed.

## Delivery Evidence

Identify the Flutter app/state/platform/API decision and the widget/provider/bloc/platform assertion proving it. Analysis success or one screenshot cannot prove lifecycle, state reconciliation, platform configuration, accessibility, or runtime API binding.

## Unsafe Defaults

- Riverpod or Bloc introduced without selected stack and state ownership.
- Services/controllers/futures constructed in `build()`.
- Watched state mutated in place or rows keyed by index.
- `BuildContext` retained across async operations without lifecycle safety.
- Device/emulator URLs and secrets hardcoded in source.
- Platform behavior reduced to visual differences without input/permission/lifecycle handling.
