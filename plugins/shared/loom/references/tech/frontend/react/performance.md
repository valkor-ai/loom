# React Performance Quality

This file applies React performance discipline when the task changes render cost, bundle cost, large collections, expensive calculations, or responsiveness.

## When To Use

- The task changes large lists, tables, dashboards, charts, expensive filtering/sorting, lazy-loaded areas, bundle-heavy dependencies, or responsiveness under frequent input.
- Use this when performance is a stated requirement, a known risk, or likely to regress because of the changed UI structure.
- Do not optimize purely for style. Keep performance changes tied to current task behavior, measured risk, or visible user impact.

## Implementation Focus

- Start with component structure and state ownership. Avoid re-rendering a whole page when a row, filter, drawer, or small control changes.
- Use `memo` for child components only when parent churn and prop stability make it useful. Do not memoize every component by default.
- Use `useMemo` for expensive derived values such as large filtering, sorting, grouping, totals, or chart data. Keep dependencies complete and easy to inspect.
- Use `useCallback` for callbacks passed to memoized children, subscription APIs, or custom hooks that require stable identity.
- Virtualize large lists or tables when the row count can become large enough to affect rendering. Do not virtualize small lists merely because a table exists.
- Code split heavy routes, admin panels, charts, editors, maps, or optional integrations with `lazy` and `Suspense` when the framework and repository convention support it.
- Use `useTransition` or deferred updates for non-urgent filtering/search updates that can lag behind input.
- Keep inline object/function churn out of hot row renderers and memoized child props.
- Avoid importing large utility libraries or chart packages into the main bundle when only one feature needs them.
- Preserve accessibility and business states when optimizing. Do not remove labels, focus behavior, loading states, or error feedback for speed.

## Verification Focus

- Run focused build/type/lint/test commands and inspect bundle or performance tooling only when the repo already provides it or the task explicitly owns performance.
- Test large-list behavior with enough rows to prove stable keys, action targeting, and virtualization or pagination behavior.
- Verify that memoization does not freeze stale props, stale callbacks, or stale validation state.
- For code splitting, verify the lazy path renders loading and error states and that the route still works after build.

## Evidence Focus

- In the evidence summary, name the performance decision: state locality, memo boundary, expensive derived data, virtualization, code splitting, transition/deferred update, bundle containment, or measured non-regression.

