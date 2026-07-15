# Vue Component, Store, And Composable Testing

Use this reference only when the task explicitly owns Vue test implementation. Keep Vue Test Utils/component/store/composable verification here; browser automation references own end-to-end navigation, real viewport/browser evidence, and artifacts.

## Choose The Public Boundary

Test pure logic directly, composables through their public refs/commands and lifecycle, stores through isolated Pinia, components through user-visible behavior/emits/models, and route/Nuxt integration through the repository harness.

Use the established runner, SFC transform, DOM environment, Vue Test Utils/testing library, request mocks, and fixtures. Do not add another runner or test stack for one task.

Tests should fail for a product/component contract regression rather than an internal refactor.

## Mounting And Queries

Mount with required router, Pinia, i18n, UI library, query/client, theme, auth, injection, and Nuxt app context. Keep shallow mounting only when child rendering is irrelevant; stubbing everything can erase the behavior under test.

Prefer accessible role/name/label/text queries where the repository supports them. Test IDs and CSS classes are fallbacks, not substitutes for semantic controls.

Assert rendered state, focus/disabled behavior, exact emitted/model payload, stable command target, and final readback.

## Components And Models

Cover prop defaults/nullability, slot content/props, emits, `v-model` updates/modifiers, invalid intermediate form values, Teleport overlays, and async component failure when owned.

Attach Teleport targets or stub deliberately according to what is being proved. Verify dialog focus/close/return behavior at an appropriate integration layer.

Avoid large snapshots for dynamic workflows. Small snapshots may protect stable generated markup only when local convention supports them.

## Composables And Lifecycle

Run composables inside a component/effect scope when they use inject, lifecycle hooks, watchers, or cleanup. Calling the function directly may never execute the real ownership lifecycle.

Change reactive inputs and assert cancellation/stale-result prevention, cleanup on unmount, and public states/actions. Do not assert private watcher counts unless performance is the contract.

Use fake timers only for timer-owned behavior and always restore them.

## Pinia And Persistence

Create a fresh Pinia/store/query cache per test. Use testing Pinia carefully: stubbed actions cannot prove action side effects or transitions.

Exercise selection changes, duplicate submit, optimistic rollback, cache invalidation/readback, persistence hydration/migration, and identity cleanup when owned.

Reset local storage, service workers/native mocks, and module singletons so tests cannot pass from prior state.

## Router, Nuxt, And Async Data

Test params/query changes, navigation intent, guards/middleware, not-found/forbidden states, and return context through the repository router/Nuxt utilities.

Mock at the accepted HTTP/client/server boundary and assert method/path/query/body/status/error mapping. Wait for visible outcomes or flushed promises rather than arbitrary sleeps.

Keep SSR/hydration/Nitro/runtime config evidence in Nuxt build/integration tests; a jsdom component mount cannot prove server/client separation.

## Type Contracts

Pair runtime tests with `vue-tsc` when props/emits/models/slots/template refs/injections/stores/plugins changed. Runtime tests do not prove template consumer types, and typecheck does not prove external input validation.

Use compile fixtures sparingly for reusable library contracts and ensure expected-error assertions remain intentional.

## Verification

- Run focused component/store/composable tests plus SFC type/build checks affected by the task.
- Prove success and meaningful validation/business/conflict/unavailable state for owned mutations/async surfaces.
- Verify exact emits/models/navigation/request targets and no unexpected calls.
- Exercise cleanup, superseded async work, isolated stores/caches, and persistence reset.
- Keep browser/native/PWA workflow claims separate from unit/component evidence.

## Delivery Evidence

Name the public behavior, harness boundary, representative state, and assertion that would fail on regression. A passing shallow mount, snapshot, or mocked action does not establish lifecycle, store transitions, routing, SSR, browser, or native behavior.

## Unsafe Defaults

- Vue testing loaded for every implementation task.
- New runner introduced despite repository tooling.
- Every child/action/router/client stubbed until no real behavior remains.
- Composables called without component/effect lifecycle.
- Shared Pinia/cache/storage leaking across tests.
- Arbitrary sleeps used for async synchronization.
- Browser/SSR/native claims made from jsdom component tests.
