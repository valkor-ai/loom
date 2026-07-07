# React Native Testing Quality

This file applies React Native testing discipline to task-owned mobile screens, components, hooks, navigation, lists, storage, and platform-specific behavior.

## When To Use

- The task creates or changes React Native behavior that should be proven with unit tests, hook tests, screen tests, navigation tests, storage tests, or platform verification.
- Use this for mobile test scope, mocked native modules, accessibility queries, navigation params, list behavior, storage hydration, and iOS/Android evidence.
- Pair with navigation, platform, lists, or storage references when those behaviors changed.

## Implementation Focus

- Test user-visible behavior through text, accessible labels, roles, press events, input changes, navigation calls, storage effects, and rendered states.
- Mock native modules deliberately and close to the test setup. Do not let a missing native mock hide the behavior under test.
- Prefer React Native Testing Library style queries when the repository uses it. Avoid brittle tree snapshots for complex screens unless snapshots are already a narrow local convention.
- Test navigation params and navigation calls through the repository's navigation helpers or mocks. Do not assert internal router state when visible behavior or navigation intent is enough.
- Test list behavior with representative data: stable key/action targeting, refresh, loading more, empty state, and error retry.
- Test storage hooks with first-run, hydrated, update, remove, corrupt value, and cleanup cases where persistence changed.
- Test platform branches by mocking `Platform.OS` or using platform-specific test files when the code path is task-owned.
- Keep async tests deterministic. Await user events, storage promises, timers, and query updates without relying on arbitrary sleep delays.
- Keep simulator/manual evidence for platform chrome, keyboard, status bar, safe area, hardware back, native permissions, and native modules that unit tests cannot fully prove.

## Verification Focus

- Run TypeScript, lint, Jest/unit tests, and repository screen/component tests affected by the task.
- Run Expo/Metro startup or equivalent build check when route files, native modules, or app config changed.
- Verify loading, empty, ready, validation error, business-blocking error, offline/error, submitting, success, disabled, keyboard, and navigation states touched by the task.
- Verify iOS and Android manually or through device automation when platform-specific behavior is changed.
- Verify tests do not depend on delivery notes, runtime instructions, or framework explanations being visible in product UI.

## Evidence Focus

- In the evidence summary, name the React Native proof: screen behavior, hook behavior, navigation intent, list action targeting, storage hydration, platform branch, native module mock, or device/simulator coverage.
