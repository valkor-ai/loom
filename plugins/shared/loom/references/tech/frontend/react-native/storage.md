# React Native Client Storage

Apply storage guidance only when the task owns device persistence, secure credentials, offline drafts, cached records, remembered preferences, persisted state, hydration, migration, expiry, or identity-scoped cleanup.

## Classify The Data

For each value, name sensitivity, authority, size, access frequency, lifetime, identity/tenant/environment scope, offline requirement, expiry, and conflict behavior before choosing a backend.

Use platform secure storage/keychain/keystore through the repository abstraction for tokens or sensitive credentials. AsyncStorage fits small non-sensitive async values; MMKV or another synchronous store fits frequent reads only when already compatible and justified.

Files/databases suit larger structured/offline data. Do not turn key-value storage into an unbounded database or use local persistence as authoritative server state.

## Keys And Stored Shape

Centralize and namespace keys by app/environment, feature, schema version, and identity dimensions as needed. Avoid one global `user`, `settings`, or `draft` key shared across accounts.

Store a versioned envelope when data evolves:

```ts
type StoredDraft<T> = {
  schemaVersion: 2
  ownerId: string
  updatedAt: string
  expiresAt?: string
  value: T
}
```

Validate parsed values at the boundary. Corrupt JSON, partial writes, unknown versions, missing fields, expired data, and downgrade scenarios need explicit discard, migrate, or recovery behavior.

## Hydration And Rendering

Represent unhydrated, ready, missing, invalid/migrating, and failed states. Do not briefly render a default as persisted truth and then replace it after storage resolves.

Sequence hydration and writes. A slow initial read must not overwrite a user change made before it completes; key/account changes must invalidate older completions.

Avoid using a new object/function default as a hook dependency that restarts hydration every render. Keep storage hooks typed and expose meaningful error/retry/reset behavior.

## Writes And Consistency

Define whether UI updates before or after durable write and what happens on write failure. For important drafts/settings, surface failure and retain recoverable in-memory state rather than pretending persistence succeeded.

Serialize writes per key or use storage transactions/batches where available. Concurrent functional updates must read from one current owner, not a stale closure.

Limit and expire caches/drafts/history. Define eviction and storage-pressure behavior; mobile OS cleanup and unavailable storage are normal failure modes.

## Identity And Lifecycle Cleanup

Clear or re-scope data on logout, account/tenant/environment switch, permission downgrade, schema migration, and app reset. Do not call backend-wide `clear()` when the app shares storage with unrelated features.

Persist only durable slices. Loading flags, transient errors, open overlays, in-flight mutations, and mutable selected rows should not survive restart by default.

For offline edits, define sync identity, conflict detection, retry/idempotency, tombstones, and readback. A cached DTO plus later overwrite is not an offline architecture.

## Security And Privacy

Assume non-secure storage, logs, backups, and device files can be inspected. Do not store raw passwords, private keys, unrestricted provider payloads, or secrets in AsyncStorage/MMKV.

Minimize sensitive retention, redact diagnostics, and follow platform backup/screenshot/data-protection policy where required. Biometric gating does not automatically encrypt arbitrary app storage.

## Verification

- Test first run, valid hydration, missing/corrupt/expired/unknown-version data, migration, update, removal, and write failure.
- Prove a late hydration cannot overwrite a newer write or another account/key.
- Verify logout/account/tenant/environment cleanup without deleting unrelated keys.
- Confirm sensitive values use the accepted secure backend and are absent from ordinary storage/logs.
- Exercise restart/offline/reconnect/conflict behavior when the task owns offline persistence.

## Delivery Evidence

Name the data class, backend, key namespace, schema/migration, hydration/write ordering, cleanup triggers, and assertions proving failure safety. A successful `setItem`/`getItem` round trip does not establish identity isolation, security, migration, or concurrency correctness.

## Unsafe Defaults

- MMKV or AsyncStorage selected from speed alone.
- Generic keys shared across accounts/environments.
- `JSON.parse` output trusted without shape/version validation.
- Default state shown as persisted truth before hydration.
- Slow hydration allowed to overwrite current user input.
- Whole storage cleared on logout.
- Server records persisted as offline truth without sync/conflict policy.
