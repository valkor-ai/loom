# Angular NgRx Quality

This file applies NgRx discipline to task-owned stores, actions, reducers, selectors, effects, entity adapters, facades, and component-store integration.

## When To Use

- The task creates or changes NgRx store setup, feature state, action groups, reducers, selectors, effects, entity collections, facade APIs, or component-store usage.
- Use this when state must be shared across routes, survive component teardown, coordinate multiple API calls, drive permissions/eligibility, or support complex list/detail workflows.
- Do not introduce NgRx for small local component state when signals, component state, or an existing lightweight store is the repository norm and the workflow does not need shared state.

## Implementation Focus

- Model feature state around domain workflow, not around API endpoints. Include loading, error, selected ID, filters, pagination, dirty draft, or optimistic state only when the UI actually uses it.
- Prefer action groups with clear source names and event names. Include enough payload context to avoid relying on stale selected state inside effects.
- Keep reducers pure and immutable. Never mutate arrays, entities, nested draft state, or error fields directly.
- Use entity adapters for normalized collections when the feature has list/detail lookup, update/delete, pagination merge, or stable identity requirements.
- Keep selectors small, typed, and composable. Derive view models in selectors or facades when multiple components need the same business-ready shape.
- Use effects for async work, external services, router side effects, and persistence. Pick effect flattening operators by business semantics: latest list load, ordered save queue, duplicate-submit prevention, or independent concurrent operations.
- Convert store selectors to signals at component boundaries when that matches the app style. Do not dispatch actions from presentational components.
- Use a facade when it reduces repeated store wiring, hides action names from components, or centralizes workflow commands. Do not create a facade that only renames one selector and one dispatch.
- Keep store keys, feature names, and selector names stable. Avoid breaking persisted state, router-store links, or devtools inspection without an explicit migration.

## Verification Focus

- Test reducers for state transitions, entity adapter behavior, error clearing, and immutable updates.
- Test selectors for filtered, sorted, selected, empty, loading, and permission-derived view models.
- Test effects for success, failure, retry, cancellation, duplicate-submit prevention, and dispatch/no-dispatch side effects.
- Verify component integration dispatches actions with the displayed record's identity and renders store-derived states correctly.
- Run build/typecheck to catch feature-key, selector, action payload, and effect typing errors.

## Evidence Focus

- In the evidence summary, name the NgRx decision: feature state shape, action payload contract, entity adapter, selector view model, effect concurrency operator, facade boundary, or store integration proof.
