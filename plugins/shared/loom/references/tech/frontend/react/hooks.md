# React Hooks Quality

This file applies React hook discipline to task-owned custom hooks, effects, subscriptions, browser integrations, and hook-based stateful components.

## When To Use

- The task creates or changes `useEffect`, `useMemo`, `useCallback`, `useRef`, `useReducer`, custom hooks, debounced search, media queries, local storage, subscriptions, timers, or browser event listeners.
- Use this when stale closures, missing cleanup, dependency arrays, or cancellation behavior can affect correctness.
- If the task only renders static markup with no hooks, do not expand scope because this reference exists.

## Implementation Focus

- Keep effects scoped to a single concern: data fetch, subscription, DOM listener, timer, analytics event, or synchronization. Split unrelated effects instead of building one large effect with hidden dependencies.
- Include every value used by an effect in the dependency list unless the value is intentionally stable by contract. Do not silence hook lint warnings to make the task pass.
- Add cleanup for subscriptions, event listeners, timers, observers, animation frames, and async work that can complete after unmount.
- For fetches or replaceable async work, use `AbortController`, a request token, or a cancellation flag so stale results do not overwrite newer UI state.
- Use functional state updates when the next value depends on the previous value, especially inside timers, async callbacks, subscriptions, and memoized callbacks.
- Use `useCallback` only when a stable function identity is needed for memoized children, effect dependencies, event unsubscription, or a custom hook contract.
- Use `useMemo` only for expensive calculations, stable object identities required by memoized children, or derived values that would otherwise cause downstream churn.
- Store mutable non-render data in refs: timers, DOM nodes, previous values, external instances, and in-flight request ids. Do not put data in refs when the UI must re-render from it.
- Custom hooks should expose a small typed contract with clear loading, error, ready, and action states when they own async work.
- Guard browser-only APIs for SSR or pre-rendered environments when the repository can run outside the browser.

## Verification Focus

- Run hook/component tests that prove effect cleanup, debounced timing, cancellation, subscription disposal, and dependency-driven refresh when touched.
- Use fake timers only where the repo already supports them; restore real timers after the test.
- For local storage or media query hooks, test missing browser APIs or SSR-safe initialization when the framework can render on the server.
- For custom hooks used by multiple components, add a hook-level test or component integration test around the public hook contract.

## Evidence Focus

- In the evidence summary, name the hook decision: effect split, cleanup path, dependency handling, stale-result prevention, functional update, memo boundary, ref ownership, or SSR browser guard.

