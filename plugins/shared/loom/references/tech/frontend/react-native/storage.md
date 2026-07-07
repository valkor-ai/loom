# React Native Storage Quality

This file applies React Native storage discipline to task-owned AsyncStorage, MMKV, SecureStore, persisted Zustand state, storage hooks, and local cache behavior.

## When To Use

- The task creates or changes mobile persistence, preferences, auth/session storage, local caches, offline drafts, feature flags, remembered filters, or storage-backed hooks.
- Use this when storage choice, serialization, lifecycle, security, cache invalidation, or hydration behavior affects the delivered workflow.
- Keep API fetching and server cache strategy aligned with the repository's data layer; do not use local storage as a substitute for authoritative server state.

## Implementation Focus

- Choose storage by data sensitivity and access pattern. Use SecureStore or the repository's secure mechanism for tokens/secrets, AsyncStorage for small async preferences, and MMKV for frequent synchronous reads when already supported.
- Keep storage keys centralized and namespaced by feature/account/environment when collisions are possible.
- Serialize and parse defensively. Invalid JSON, old schema versions, missing values, and app upgrades need safe fallbacks.
- Keep hydrated loading state explicit. Screens should not briefly render incorrect defaults as real data while storage is still loading.
- Avoid storing mutable server records as authoritative truth unless the feature explicitly supports offline mode and conflict handling.
- For persisted stores, define which slices persist and which slices remain runtime-only. Do not persist loading flags, transient errors, selected modal state, or stale form submission state.
- Use small typed hooks or services for storage operations. Components should not scatter raw storage calls across event handlers.
- Clear or migrate storage on logout, account switch, environment switch, permission downgrade, or schema change when data scope changes.
- Avoid unbounded storage growth for caches, drafts, logs, or recent activity. Add limits, expiry, or invalidation when persistence can grow.

## Verification Focus

- Verify first-run empty storage, existing value hydration, invalid/corrupt value fallback, update, removal, logout/account-switch cleanup, and schema migration when in scope.
- Verify sensitive values are not written to non-secure storage.
- Verify persisted UI preferences do not override server/business state incorrectly after refresh or relaunch.
- Verify hooks expose loading/error states and do not update unmounted components after async storage calls resolve.
- Run the repository's focused tests for storage hooks, stores, and serialization helpers when available.

## Evidence Focus

- In the evidence summary, name the storage decision: storage backend, key namespace, secure handling, hydration state, parse fallback, persisted slice boundary, cleanup trigger, or cache invalidation proof.
