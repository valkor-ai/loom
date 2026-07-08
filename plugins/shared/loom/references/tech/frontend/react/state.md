# React State Quality

This file applies React state-management discipline to task-owned local state, context, reducers, external stores, server-state caches, and form/action state.

## When To Use

- The task creates or changes local UI state, selected records, modal/drawer state, form drafts, filters, sorting, pagination, optimistic updates, global state, or server-state cache usage.
- Use this when the feature must keep UI state, API data, and business action state consistent.
- If the task only changes presentational styling with no state ownership, keep this reference out of scope.

## Implementation Focus

- Keep state ownership as local as possible. Use local component state for isolated UI concerns, context for small cross-tree concerns, a store for shared workflow state, and a server-state library for cached remote data.
- Separate server data, editable form drafts, selected row snapshots, optimistic state, and view filters when they have different lifecycles.
- Do not derive submitted payloads from a different object than the one currently shown in a modal, drawer, or detail panel.
- Use reducers for related multi-step state transitions, state machines, or action-driven workflows where separate `useState` calls can drift.
- Use Context sparingly for low-frequency shared state. Memoize provider values and split contexts by update frequency when a provider feeds many children.
- Use Zustand, Redux Toolkit, or the repository's existing store only when local state and context are insufficient. Do not add a new state library for a single small task.
- Treat TanStack Query or similar libraries as server-state tools. Keep mutation invalidation, optimistic updates, and error recovery explicit.
- Keep business-blocking results separate from technical failures: validation error, conflict, forbidden action, stale data, and network failure should be distinguishable when the API contract distinguishes them.
- Persist state only when the requirement or existing UX needs it. Validate persisted values before trusting them.
- Reset or preserve form state deliberately when modal open target, route params, selected row, or authenticated user changes.

## Verification Focus

- Test state transitions that can break user trust: select row then submit, edit then cancel, retry after failure, submit twice, change filters while request is pending, and close/reopen modal.
- Test reducers as pure functions when business logic lives there.
- For store-backed flows, test selector behavior and action effects without coupling tests to implementation-only store internals.
- For server-state caches, verify cache invalidation or refetch after successful mutations and visible error handling after failed mutations.

## Evidence Focus

- In the evidence summary, name the state ownership decision: local state, reducer, context, store, server-state cache, form draft separation, selected-record snapshot, optimistic update, or persisted state validation.

