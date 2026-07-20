# React Hooks And Reactive Client Lifecycles

Apply hooks guidance when the task owns effects, subscriptions, browser integrations, timers, replaceable async work, custom hooks, or another reactive client flow. Static/presentational component work should not load it.

## Effects Are Synchronization

Use effects to synchronize React state with systems outside rendering: DOM/browser APIs, subscriptions, timers, analytics, imperative widgets, or manual data sources. Do not use effects to compute values from props/state or mirror one state variable into another.

Keep one concern per effect and include all reactive dependencies. Restructure unstable objects/functions or move logic rather than suppressing exhaustive-deps.

```tsx
useEffect(() => {
  const controller = new AbortController()
  void search(query, { signal: controller.signal }).then(setResults, error => {
    if (!controller.signal.aborted) setFailure(mapError(error))
  })
  return () => controller.abort()
}, [query])
```

Handle development Strict Mode setup-cleanup replay; an effect must be safely repeatable and cleanup must undo setup.

## Cleanup And Stale Work

Remove listeners/observers/subscriptions, clear timers/animation frames, abort requests, dispose external instances, and prevent stale completions overwriting newer state.

Mounted flags can prevent updates but do not cancel work. Prefer real cancellation/token sequencing where available and define latest/order/duplicate behavior.

Use functional updates when callbacks depend on prior state. Refs hold mutable non-render values such as DOM nodes, timer IDs, previous values, and external instances; UI-visible state belongs in state/reducer/store.

## Custom Hooks

Extract hooks for reusable stateful behavior or to isolate a complex external lifecycle, not merely to move code. Keep typed input/output contracts small and expose state/actions rather than implementation internals.

Custom hooks obey hook rules and should not conditionally call hooks. Avoid hidden global singletons, implicit routing, or broad API/error behavior inside a generic hook.

For async hooks, expose meaningful idle/loading/ready/empty/error/refreshing/mutating state and cancellation/retry semantics. Do not return only `data | null` when failures matter.

## Memoization

Use `useMemo` for measured expensive derivation or required stable identity, `useCallback` for consumers that rely on function identity, and `memo` at proven component boundaries.

Memoization is not semantic correctness and can retain stale dependencies/objects or cost more than recomputation. Do not wrap every handler/value.

React Compiler or framework optimization may change manual memo needs; follow accepted tooling/version and verify behavior/performance rather than deleting/adding memo mechanically.

## Browser APIs And SSR

Initialize browser-only values lazily/effect-side with a deterministic server/first render when SSR/pre-rendering is possible. Guard storage, media queries, ResizeObserver, window/document, and third-party widgets.

Storage events, media subscriptions, and external stores need `useSyncExternalStore` or a correct subscription snapshot contract when multiple components must stay consistent.

Validate persisted data and clear/re-scope it on identity/tenant/schema changes. Browser storage is not secure storage.

## Debounce, Timers, And Events

Debounce/throttle based on product behavior and cancel pending work on unmount/input changes. Avoid recreating debounced functions each render or closing over stale values.

Event handlers are preferable to effects for user-caused operations. Do not set a flag then use an effect solely to notice the flag and submit.

## Verification

- Test setup/cleanup replay, unmount disposal, and dependency-driven resubscription.
- Prove stale request/timer results cannot overwrite newer state.
- Verify debounce/throttle timing and cancellation with controlled timers only when owned.
- Test SSR/missing browser API, persisted-data validation, identity clearing, and external-store updates.
- Exercise custom hook public state/actions, not private refs/effect count alone.

## Delivery Evidence

Name the external system/lifecycle and cleanup/cancellation assertion proving it. A complete dependency array or passing happy-path hook render cannot prove stale-result safety, strict-mode replay, resource disposal, or SSR behavior.

## Unsafe Defaults

- Effects used for derived values or user-event commands.
- Hook lint disabled instead of fixing dependencies.
- Mounted flags treated as cancellation.
- Refs used for UI state or mutable globals hidden in hooks.
- Universal useMemo/useCallback/memo.
- Browser APIs read during SSR initial render without a stable fallback.
