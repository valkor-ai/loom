# Flutter Testing Quality

This file applies Flutter testing discipline to task-owned widgets, providers, blocs, routes, services, generated models, and cross-platform workflow behavior.

## When To Use

- The task creates or changes Flutter behavior that should be proven with widget tests, provider tests, bloc tests, route tests, service tests, golden tests, or integration tests.
- Use this for test scope, test doubles, async pumping, provider/bloc overrides, navigation verification, platform behavior, and evidence quality.
- Pair with focused Flutter references for widgets, Riverpod, Bloc, navigation, or performance when those implementation areas changed.

## Implementation Focus

- Test user-visible behavior through widgets, text, semantics, inputs, taps, scrolls, validation messages, route changes, and rendered state.
- Use `WidgetTester` and `pump`/`pumpAndSettle` deliberately. Avoid arbitrary delays that hide unstable async behavior.
- Use provider overrides, fake repositories, fake clocks, fake storage, and fake permission services so tests do not hit real infrastructure.
- Test Riverpod providers through provider containers or Consumer widgets depending on whether state logic or rendering behavior is the risk.
- Test Bloc/Cubit flows with initial state, event/command, emitted states, and side-effect listener behavior.
- Test route behavior with router configuration and stable location/stack assertions when navigation changes are in scope.
- Test generated serialization/model changes with representative valid, missing, invalid, and backward-compatible data.
- Use golden tests only when the repository already maintains them or visual regression risk is high. Do not introduce brittle goldens for ordinary logic-only changes.
- Keep test fixtures small and domain-specific; do not build giant mock app state when the workflow needs only a few fields.

## Verification Focus

- Run `flutter analyze`, focused `flutter test`, and coverage/profile/build commands when required by task risk or repository policy.
- Cover loading, empty, ready, validation error, business-blocking error, submitting, success, disabled, offline, navigation, and platform states touched by the task.
- Verify async work settles deterministically and tests fail for the right reason when API/storage/state errors occur.
- Verify generated files are up to date when annotations, Freezed classes, JSON models, or Riverpod generated providers changed.
- Verify tests do not depend on delivery notes, runtime instructions, or framework explanations being visible in product UI.

## Evidence Focus

- In the evidence summary, name the Flutter test proof: widget behavior, provider state, bloc transition, route behavior, service contract, generated model, platform behavior, golden, or integration coverage.
