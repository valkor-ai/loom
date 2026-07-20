# Flutter Testing

Use Flutter framework testing only for tasks that own tests. Choose the smallest proof for the changed boundary. Browser/device integration references own true end-to-end runtime, platform, and rendered multi-viewport evidence when separately assigned.

## Proof Boundary

| Claim | Suitable proof |
|---|---|
| Pure validator/mapper/use case | Dart unit test |
| Riverpod notifier/provider | `ProviderContainer` test |
| Bloc/Cubit transitions | bloc test/unit test |
| Widget rendering/interaction | `testWidgets` |
| Router stack/redirect | widget/router test |
| Plugin/platform channel adapter | adapter test plus selected platform integration |
| Full app workflow/real service | integration/device/browser task |

Do not boot the whole app for every pure rule or claim a widget test proves native permission dialogs, deep-link OS configuration, web hosting fallback, or deployed API binding.

## Widget Harness

Build a focused test app with required theme, localization, MediaQuery, router, and provider/bloc overrides. Avoid giant production app fixtures that make unrelated dependencies part of every test.

Interact through finders, semantics, text fields, taps, scrolls, keyboard actions, and visible output. Do not mutate private State fields to manufacture results.

Use `pump` for controlled frames and `pumpAndSettle` only when animations/timers actually settle. Repeating timers, infinite animations, polling, or unresolved streams can make `pumpAndSettle` hide/hang; pump expected durations/events explicitly.

## State And Dependency Tests

Override repositories, clocks, storage, permissions, network/connectivity, and platform ports with deterministic fakes. Assert target ID, payload, calls, durable state mapping, and absence of forbidden side effects.

Riverpod tests should create/dispose isolated containers and verify notifier state/lifetime/invalidation. Bloc tests should assert exact state sequence/concurrency and close instances.

Do not mock the notifier/bloc/widget behavior being claimed. A mocked state stream can be useful for widget rendering but is not proof of the state implementation.

## Forms, Lists, And Semantics

Cover task-owned loading, empty, ready, validation, business conflict, permission, offline/unavailable, submitting, success, disabled, and destructive confirmation states.

For lists, test stable target identity after sort/filter/page/refresh and verify scroll/pagination deduplication where owned. For forms, verify draft preservation, field/global errors, focus, duplicate submit, and server readback reconciliation.

Use semantics tests for labels, roles/actions, toggled/disabled state, error announcements, hit targets, and custom controls. Exercise text scaling, narrow/wide constraints, long/localized content when layout changes.

## Navigation And Platform

Router tests should prove initial location, parameters, push/replace/back, shell/tab stacks, redirects, invalid/not-found/forbidden, and listener deduplication. Platform deep links/cold starts require integration evidence.

Abstract method channels/plugins for unit/widget tests and add selected-target integration tests when manifest/plist/permissions/native behavior is the claim. Do not use one platform fake as proof all target setup works.

## Golden And Integration Tests

Use goldens only where repository tooling maintains fonts, device pixel ratio, themes, localization, and update/review workflow, and visual regression risk justifies them. Goldens do not prove interaction/accessibility.

Integration tests need deterministic seed/account/data, explicit waits for observable state rather than sleeps, and cleanup. Separate environment/runtime failures from code failures.

## Generated Models And Serialization

Test valid/missing/null/unknown/backward-compatible JSON/storage data and enum/date/decimal/version mapping when models change. Regenerate before testing and never patch generated output to satisfy a test.

## Verification And Cleanup

Run the changed test file first, then focused `flutter analyze`/owning package tests/build only when shared types, routes, generated code, or platform config changed. Do not impose a universal coverage threshold absent repository policy.

Dispose containers/blocs/controllers/fakes, restore platform dispatchers/overrides, clear timers, and verify pending exceptions. Arbitrary sleeps and test-order dependence are defects.

## Delivery Evidence

Record boundary, scenario, command, and meaningful widget/state/route/platform assertion. Passing counts, coverage, private state, or one golden cannot prove lifecycle, navigation, platform setup, responsive semantics, or real integration.

## Unsafe Defaults

- Load this reference only when the accepted task owns Flutter test creation, test modification, or test-specific verification.
- Whole-app widget tests for every pure rule.
- Private State mutation or mocked state claimed as implementation proof.
- `pumpAndSettle`/arbitrary sleeps used without understanding pending work.
- Goldens used for interaction or accessibility claims.
- One platform fake claimed as native configuration evidence.
- Universal coverage requirement copied from external guidance.
