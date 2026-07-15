# Vue State And Pinia

Apply state guidance only when the task owns shared client state, API-backed state, client persistence, selected/draft workflow state, optimistic transitions, or store lifetime. Static/presentational Vue work should not create a store.

## Choose The Owner

Keep state local to a component/composable when one surface owns it. Use a Pinia store for cross-component/route workflows, authenticated identity, shared commands, or state with an application-level lifetime.

Keep remote cached data in the repository's query/data layer when present. Do not duplicate the same resource in an effect ref, query cache, and Pinia store without a defined source of truth.

Separate server records, editable drafts, selected snapshots, filters/page, pending commands, validation errors, optimistic overlays, and persisted preferences according to lifetime.

## Store Design

Preserve the repository's setup/options store convention. Organize stores by product capability and ownership rather than one store per component or one application-wide mega-store.

State is serializable/inspectable where practical; getters derive values; actions own transitions and side effects shared by consumers. Presentational toggles usually remain local.

Use `storeToRefs` when destructuring reactive state/getters; actions can be destructured directly. Avoid copying store refs into separate refs that drift.

Return readonly state from composables/providers when callers should use commands rather than direct mutation.

## API And Async State

Model idle/loading/refreshing/ready/empty/error/mutating states without erasing usable data during refresh. Preserve distinct validation, forbidden, not-found, conflict, unavailable, and unexpected failures where user action differs.

Key requests/cache by every resource, filter, page, tenant, identity, and locale dimension affecting results. Cancel or sequence superseded requests and prevent old responses from replacing a newer route/filter/account.

Actions receive stable target identity in their payload. Never derive a mutation target from mutable global selection after confirmation or async delay.

After mutation, reconcile returned ID/version/status/normalized values and invalidate/refetch only affected resources. A success toast without visible readback is incomplete.

## Optimistic Transitions

Use optimistic state only when reversible and understandable. Track operation identity and prior state so overlapping writes can reconcile independently.

Define success, validation rejection, authorization failure, conflict, network uncertainty, out-of-order completion, and server-normalized response behavior.

Do not let an optimistic client transition authorize an action or permanently hide a failed command.

## Store Lifetime And SSR

Create isolated Pinia instances per application/request/test. In SSR/Nuxt, never use a process-global mutable store that leaks data across requests/users.

Hydrate only serializable intended state and avoid server/client divergence from browser-only values. Identity/account changes must reset or re-scope incompatible stores and caches.

Dispose store-created watchers/subscriptions when their owner ends. Component `storeToRefs` cleanup does not automatically stop watchers created in a long-lived store.

## Persistence

Persist only explicit durable slices. Version and validate stored data, namespace by identity/environment, represent hydration, and define migration/expiry/logout/account-switch cleanup.

Do not persist loading flags, transient errors, open dialogs, in-flight operations, or sensitive data to ordinary browser/device storage. A persistence plugin still needs schema and identity policy.

## Cross-Store Dependencies

Keep dependency direction clear and avoid cyclic initialization/action chains. Pass data into actions or extract a lower-level service when two stores would otherwise call each other recursively.

For plugins/subscriptions, define ordering, error behavior, and disposal. Do not hide business commands in generic persistence/logging plugins.

## Verification

- Test getters/actions/transitions through isolated active Pinia instances.
- Prove request/cache dimensions, supersession, targeted invalidation, and no cross-user/request leakage.
- Exercise selection changes, duplicate submit, optimistic overlap/rollback, conflict, and returned readback.
- Test persisted missing/corrupt/old/expired data plus logout/account/environment cleanup when owned.
- Verify store watcher/subscription disposal and SSR hydration consistency.

## Delivery Evidence

Name the state owner/lifetime/key/action, transition table, and assertion proving visible consistency. Store existence or a successful fetch does not prove isolation, target correctness, invalidation, rollback, persistence, or SSR safety.

## Unsafe Defaults

- Pinia introduced for local one-component state.
- Remote data duplicated across refs, Pinia, and query cache.
- Store actions reading mutable selected state instead of payload targets.
- Request keys omitting identity/filter dimensions.
- Optimistic updates without operation identity and rollback.
- Persisted or SSR state shared across users/tests.
- Long-lived store watchers never disposed.
