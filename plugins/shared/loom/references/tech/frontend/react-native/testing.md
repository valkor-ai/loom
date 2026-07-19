# React Native Verification

Use this reference only when the task explicitly owns React Native test implementation. Component/screen/hook tests establish JavaScript-visible behavior; native builds, simulators/devices, and mobile automation establish platform integration that unit tests cannot.

## Select The Evidence Layer

Use pure tests for validators/reducers/formatters, hook tests for public lifecycle/state contracts, React Native Testing Library for component/screen behavior, navigation/provider integration tests for composed flows, and device/native checks for system APIs and platform chrome.

Do not claim safe-area, keyboard, permission prompts, deep links, status bars, native modules, gestures, performance, or production signing from a mocked DOM-like tree alone.

Use the repository's runner, transforms, presets, renderer, mocks, and test utilities. Do not introduce another test stack because an external example uses Jest or a specific library.

## Queries And Interaction

Prefer accessible role/name/label/text/state queries and realistic press/type/scroll interactions supported by the test library. Test IDs are a fallback for elements without a meaningful semantic locator.

Assert visible state and exact emitted command/navigation target. For lists and actions, change sort/filter/selection/refresh before activating a row to catch mutable-target defects.

Cover draft preservation, field/global errors, duplicate-submit blocking, disabled/forbidden behavior, success readback, and retry when task-owned.

## Harness And Isolation

Create isolated router/navigation, query cache, store, theme, i18n, auth, safe-area, and feature-flag providers per test. Avoid shared singletons leaking identity or cached records.

Mock the accepted API/native/storage boundary, not every hook/component under test. Reject unexpected calls and assert method/path/payload/params when they are part of the contract.

Reset module mocks, timers, storage, stores, query clients, permission state, and `Platform.OS` changes after each case.

## Navigation And Deep Links

Test route param parsing, direct entry, push/replace/back intent, protected routing, modal dismissal, and return-context behavior using the repository harness.

Keep actual URL-scheme/association and cold/warm deep-link validation at the platform integration layer. A mocked router call cannot prove operating-system delivery.

## Storage And Async Work

Exercise missing, valid, corrupt, expired, migrating, failed, and identity-switched storage states when owned. Prove late reads/requests do not overwrite newer state.

Await visible outcomes rather than arbitrary sleeps. Use fake timers only for timer-owned behavior and restore them. Trigger cleanup by unmounting, blurring, navigating, or changing dependencies as the public lifecycle requires.

## Native Modules And Permissions

Provide explicit native-module mocks that mirror the relevant success/failure contract. A permissive empty mock can conceal missing installation or unsupported method behavior.

Test permission state transitions in JavaScript, then verify real prompt/config/capability behavior on the affected platform when the task changes native permissions.

## Lists And Performance

Use representative collection data to prove key/action identity, refresh, pagination guards, errors, and recycled-row behavior. Render-count assertions are appropriate only for an explicit performance task and must preserve correctness.

Profile actual mobile runtime for list, animation, memory, image, startup, or interaction performance. Unit tests cannot establish frame rate or native memory behavior.

## Platform Coverage

Run platform-specific builds/simulators/devices according to task ownership and available environment. If one platform is unavailable, record the precise limitation and remaining platform risk; do not fail unrelated code work or claim both-platform coverage.

An enforcement requirement may make missing platform evidence a delivery blocker, but generic mobile tasks do not automatically require every device matrix.

## Verification

- Run focused type/lint/unit/screen tests and the affected Metro/native build boundary.
- Cover success plus meaningful validation, authorization, conflict, offline/native failure, or unavailable state owned by the task.
- Verify provider/store/storage isolation and cleanup between cases.
- Execute real platform checks for native config, permissions, deep links, keyboard/system chrome, or native modules when owned.
- Keep mobile automation artifacts distinct from component-test evidence.

## Delivery Evidence

Name each behavior, evidence layer, platform/runtime, and assertion. Report mocks and unavailable platforms honestly. Passing component tests do not prove native installation, OS integration, real-device capability, or performance.

## Unsafe Defaults

- Load this reference only when the accepted task owns React Native test creation, test modification, or test-specific verification.
- Snapshot trees used as the primary workflow evidence.
- Native modules mocked to empty objects regardless of real contract.
- Arbitrary sleeps used for async/animation synchronization.
- Shared router/store/storage state leaking across tests.
- Both-platform or real-device coverage claimed from one simulator.
- Missing device evidence routed as a generic source-code defect.
