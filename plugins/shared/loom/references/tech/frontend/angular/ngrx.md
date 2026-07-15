# Angular Shared State With NgRx

Apply NgRx only when TechnicalBaseline selects it and the task owns shared client state, reducers/store, selectors, effects, entity collections, or a cross-surface lifecycle. Local component state does not justify a store.

## State Ownership

Model state around product workflow and source-of-truth boundaries, not one field per API response. Keep only state required across components/routes, for coordinated effects, or for stable workflow history.

A feature state may include normalized entities, request status/error, selected identity, filters/page, dirty draft, optimistic operation, and freshness metadata when those are actually used. Do not duplicate derived lists/counts/eligibility that selectors can compute.

Keep server data ownership explicit. NgRx state is a client representation/cache, not authority over concurrent backend changes.

## Feature Registration And Keys

Use `provideState`/`provideEffects` or the repository's module setup at the correct application/route lifetime. Keep feature keys stable and unique; changing a key can break selectors, router integration, persisted state, devtools, and tests.

Register effects once. Lazy route registration needs clear teardown/re-entry behavior and must not duplicate side effects.

## Actions And Reducers

Use action groups with business/event sources and typed payloads:

```typescript
export const OrdersActions = createActionGroup({
  source: 'Orders Workbench',
  events: {
    'Load Requested': props<{ filter: OrderFilter }>(),
    'Load Succeeded': props<{ orders: readonly OrderSummary[] }>(),
    'Load Failed': props<{ error: UiError }>(),
    'Approval Requested': props<{ orderId: string; expectedVersion: number }>(),
  },
});
```

Include stable target/context in commands so effects do not read a possibly changed `selectedId` after the user navigates or filters.

Reducers are pure and immutable. Clear stale errors/loading/optimistic state on the correct initiating/success/failure events. Never mutate entity arrays, nested drafts, or error objects.

Use `createEntityAdapter` for normalized collections with stable identity and list/detail updates. Configure `selectId` and sorting only when they match domain identity and desired canonical order; pagination order may need separate ID lists.

## Selectors And Facades

Keep selectors typed, pure, composable, and free of service calls, mutation, time/randomness, or component-only formatting. Build reusable business-ready view models where multiple surfaces need them.

Avoid factory selectors created repeatedly during rendering without memoization/lifetime control. Prefer selected-ID plus entities selectors or a facade method that reuses selector instances.

A facade is useful when it hides store mechanics, centralizes commands/view models, or protects component APIs from action/key churn. Do not create pass-through facades that only rename every dispatch/select one-for-one.

Convert selectors to signals at container boundaries when selected by the repository; presentational components still receive typed values/events.

## Effects And Concurrency

Effects coordinate async/external work and dispatch outcomes. Choose flattening by operation semantics:

- `switchMap` for replaceable list/filter loads
- `exhaustMap` for duplicate-submit prevention
- `concatMap` for ordered writes
- bounded `mergeMap` for independent operations

Catch errors inside the inner operation so the effect stream remains alive. Map validation/conflict/permission/unavailable failures to typed events instead of generic strings.

Use `concatLatestFrom` only when the latest store value is truly required after the action arrives. Do not hide missing action payload context through broad state reads.

Router, toast, analytics, and other non-dispatch side effects need `dispatch: false` and should not replace product-visible state/recovery.

## Optimistic And Persisted State

Optimistic changes require temporary identity/version, rollback/reconciliation, duplicate response handling, and visible pending/failure behavior. Avoid optimistic updates for destructive/high-conflict operations without an accepted design.

Persist only explicitly safe state with schema/version/migration and logout/tenant clearing. Never persist tokens, secrets, sensitive records, transient loading/errors, or stale authorization decisions by default.

## Verification

- Test reducer transitions, immutable updates, entity adapter identity/order, and stale-state clearing.
- Test selectors for empty/loading/error, filtering/sorting, selected identity, permissions, and view-model derivation.
- Test effects for success/failure, operator concurrency, duplicate-submit, cancellation, retry bounds, and no-dispatch effects.
- Verify feature registration/key and lazy route lifecycle in an integration boundary when changed.
- Verify optimistic rollback/reconciliation and persisted-state migration/clearing when owned.
- Confirm components dispatch the displayed record identity and render selector/facade states.

## Delivery Evidence

Identify the feature key/state, action, reducer/selector/effect decision, and transition/emission assertion proving it. Redux DevTools visibility or a successful API response cannot prove immutability, action targeting, effect concurrency, rollback, or persisted-state safety.

## Unsafe Defaults

- NgRx loaded/introduced without selected stack and shared-state ownership.
- API responses copied wholesale into duplicated feature state.
- Effects reading mutable selected state instead of action target context.
- Reducers mutating nested/entity data.
- One pass-through facade method per selector/action.
- Effect streams terminating after the first error.
- Sensitive or authorization state persisted without lifecycle/migration policy.
