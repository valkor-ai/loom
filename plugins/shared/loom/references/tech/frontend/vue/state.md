# Vue State Quality

This file applies Vue and Pinia state-management rules to task-owned local state, global stores, server data, persisted state, and action-driven workflows.

## When To Use

- The task creates or changes Pinia stores, local UI state, shared workflow state, selected records, form drafts, filters, pagination, optimistic updates, or persisted settings.
- Use this when state ownership, reactivity, store boundaries, or action side effects affect the delivered behavior.
- If the task only renders static content, do not add a store because this file exists.

## Implementation Focus

- Keep state local when it belongs to one component or route. Use Pinia only for shared workflow state, authenticated user state, cross-route data, or durable product settings.
- Prefer Pinia setup stores for new work in Composition API projects. Keep options stores only when the repository already uses them consistently.
- Use `storeToRefs()` when destructuring store state or getters. Actions can be destructured directly.
- Keep API/server data, editable drafts, selected snapshots, filters, optimistic values, and persisted settings separate when they change at different times.
- Put business actions in stores only when multiple components need the same workflow. Keep purely presentational toggles local.
- Keep persistence explicit and narrow. Validate persisted values before trusting them, and avoid persisting sensitive user data unless the architecture requires it.
- Model loading, error, ready, empty, and business-blocking states explicitly. Do not collapse all failures into one string when API outcomes differ.
- Reset or preserve state deliberately when route params, selected records, authenticated user, or modal open target changes.

## Verification Focus

- Test store actions, getters, state reset, persisted-state validation, and cross-store interactions when touched.
- Test UI flows that can drift: select row then submit, edit then cancel, retry after failure, submit twice, change filters while pending, and close/reopen modal.
- Use isolated Pinia instances in tests so stores do not leak state across cases.
- Verify cache invalidation, refetch, or visible readback after successful mutations.

## Evidence Focus

- In the evidence summary, name the state decision: local state, Pinia store, `storeToRefs`, server-data split, selected snapshot, persisted state, optimistic update, or state reset proof.
