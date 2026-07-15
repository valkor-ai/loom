# React Shared And Server State

Apply this reference when the task owns shared client state or an API/server-state binding. Local ephemeral rendering state remains in the core component boundary and does not require a store/cache reference.

## Classify State By Ownership

| State | Preferred owner |
|---|---|
| Local open/input/focus/draft | component/form hook |
| Related local transitions | `useReducer` |
| Low-frequency cross-tree dependency | focused Context |
| Shared workflow/client lifecycle | selected Zustand/Redux/other store |
| Remote cached source | selected TanStack Query/SWR/data layer |
| Shareable filter/tab/page | router URL state |

Do not copy remote data into local/context/store layers without a synchronization reason. Separate persisted records, editable drafts, selected target, pending/optimistic operation, and filters when lifecycles differ.

## Local Reducers And Context

Use reducers for related event-driven transitions where many `useState` values can drift. Events include target/payload context and reducers remain pure/immutable.

Use Context for stable or low-frequency cross-tree values such as theme/session adapters, not every fast-changing feature collection. Split contexts by update frequency/ownership and memoize provider values only when it reduces real churn.

Do not hide feature business operations inside a generic app context.

## External Stores

Use only the library selected by TechnicalBaseline/repository. Define store slice ownership, normalized identity, actions/commands, selectors, initialization/reset, and route/identity lifetime.

Zustand/Redux Toolkit examples are alternatives. Do not add a store for one modal or duplicate a server-state library.

Selectors should be stable and narrow; avoid returning new objects/arrays without equality/memo behavior on hot paths. Store actions should include displayed target ID rather than read mutable selection later.

## Server-State Libraries

Define query keys from every resource/filter/page/tenant/identity dimension affecting the result. Bound stale/gc/refetch/retry/polling behavior and clear/invalidate on mutation/logout/tenant changes.

Expected validation/conflict/forbidden/unavailable failures remain typed and user-visible. Do not retry non-idempotent/business/auth failures generically.

Mutations need exact invalidation/update/readback. Optimistic changes require stable temporary/target identity, rollback, conflict, duplicate, and stale-response handling.

## Drafts, Selection, And Persistence

When opening a form/detail/modal, snapshot or key draft state to the displayed record. Reset/preserve deliberately when the target, route, identity, or server version changes.

Do not derive submit payload from a different selected object than the visible draft. Prevent stale background responses from overwriting a newer target.

Persist client state only when the product requires it. Version/validate persisted schemas and clear sensitive/authorization/transient state. Local/session storage is public to browser scripts and not authority.

## Concurrency And Derived State

Keep derived labels, filtered lists, totals, eligibility, and selected entities in selectors/computation rather than duplicated writable state.

Define latest-wins, ordered, independent, or duplicate-blocking semantics for async commands. State must return to usable conditions after error/cancellation.

Use transitions/deferred values for rendering urgency, not as a substitute for source-of-truth or request cancellation.

## Verification

- Test select-target/edit/cancel/save/reopen and target changes during pending work.
- Test reducer/store transitions, selector derivation, immutable updates, reset/logout/tenant behavior.
- Verify query keys, stale/refetch/retry, exact invalidation, readback, and no cross-user leakage.
- Prove duplicate/optimistic rollback/conflict/stale-response behavior where owned.
- Verify persisted-state validation/migration/clearing.
- Test components render states and dispatch commands for the displayed target.

## Delivery Evidence

Identify state owner/lifetime/key/action and transition/cache assertion proving it. Store/provider presence or a successful fetch cannot prove target consistency, invalidation, race handling, persistence safety, or cross-user isolation.

## Unsafe Defaults

- State reference loaded for every React component.
- Remote data duplicated across effect, context, store, and query cache.
- Store library added for small local state.
- Commands reading mutable selected state instead of payload target.
- Query keys omitting identity/filter dimensions.
- Optimistic or persisted state without rollback/schema/logout handling.
