# Flutter Core Quality

This file applies Flutter 3+ implementation discipline to task-owned applications, screens, widgets, providers/blocs, routes, services, and cross-platform UI workflows.

## When To Use

- The task creates or changes Flutter app surfaces, Dart UI code, feature screens, reusable widgets, state orchestration, navigation-connected pages, or mobile/desktop/web Flutter workflows.
- Use this for Flutter-specific widget composition, build method discipline, Material/Cupertino behavior, platform awareness, async UI states, and product workflow delivery.
- Pair with the focused Flutter reference for structure, widgets, Riverpod, Bloc, navigation, performance, or tests when the task owns those details.

## Implementation Focus

- Preserve the repository's Flutter version, state library, navigation library, folder layout, theme system, lints, code generation, and test style before introducing new patterns.
- Keep screens as workflow orchestration. Extract reusable widgets, feature widgets, providers/blocs, services, validators, formatters, and repositories when a screen becomes hard to inspect.
- Use `const` constructors and const child widgets wherever values are static. Do not create unnecessary objects in `build()` for static padding, text, icons, decoration, or child trees.
- Keep business state explicit: loading, empty, ready, validation error, business-blocking error, submitting, success, disabled, offline, and stale-state outcomes should not collapse into one generic text state.
- Use stable keys for dynamic lists, reorderable rows, animated children, forms with repeated fields, and any widget whose identity must survive filtering or refresh.
- Keep domain mutations outside pure widgets. Widgets may invoke provider/bloc commands, but repositories, storage, API clients, and permission adapters should live behind testable boundaries.
- Follow Material, Cupertino, or repository design-system conventions. Do not mix raw platform visual conventions without a product reason.
- Keep product UI free of delivery notes, runtime commands, framework explanations, verification instructions, and implementation progress text.

## Verification Focus

- Run `flutter analyze`, the repository's focused Flutter tests, and build/profile checks when the task changes app configuration, routes, native behavior, or performance-sensitive surfaces.
- Verify loading, empty, ready, validation error, business-blocking error, submitting, success, disabled, navigation, and platform states touched by the task.
- Verify widget rebuild behavior when state selection, `const`, keys, or performance changes are part of the task.
- Verify platform behavior for iOS, Android, web, desktop, or responsive layouts when the task owns platform differences.
- Verify generated code is refreshed when the task changes Riverpod annotations, Freezed models, JSON serialization, or other build-runner outputs.

## Evidence Focus

- In the evidence summary, name the Flutter decision: widget boundary, state-management boundary, route ownership, const/key optimization, platform behavior, async state contract, generated-code update, or profile/test proof.
