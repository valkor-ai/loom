# Redis Cache Integration

## When To Use

Use this reference only when the task owns the accepted Redis `cache` capability or changes a cache read/write boundary.

## Implementation Focus

- Define the authoritative source before adding a cache entry.
- Prefer cache-aside for ordinary reads: read Redis, read the source on a miss, then write the result with a TTL.
- Choose String for a complete immutable serialized value and Hash for a small object with independently updated fields.
- Keep cache keys stable, namespaced, identity-aware, and versionable.
- Set TTL on every cache entry unless the accepted design explicitly owns another eviction boundary.
- Add jitter for large groups of entries that would otherwise expire together.
- Invalidate or update cache entries after the source mutation commits.
- Invalidate list, summary, and detail keys affected by the same mutation; do not clear the whole database by default.
- Decide whether not-found results are cached and for how long.
- Treat Redis failure as a cache miss only when the accepted design permits a source read.

## Invalidation Boundary

The cache must never make a committed source mutation appear successful while returning stale authorization or ownership data. For transactional sources, perform invalidation after commit or use a proven outbox/read-model boundary.

Use a Set only when the feature needs a bounded index of related cache keys for tag invalidation. Use `SCAN` for operational cleanup; never use unbounded `KEYS` in application request paths.

## Verification Focus

- Miss then hit returns the same identity-scoped value.
- A mutation invalidates or updates every affected key after commit.
- Tenant, actor, locale, permission, and filter dimensions do not collide.
- TTL and negative-cache behavior match the contract.
- Redis unavailable falls back to the source without corrupting the response or mutation.
- Concurrent same-key loads have a bounded stampede policy.

## Evidence Focus

Name the source-of-truth method, cache adapter, key format, TTL configuration, invalidation trigger, fallback behavior, and the test proving each owned rule.

## Unsafe Defaults

- Caching a database entity whose authorization is checked only before the first write.
- Using an implicit argument serialization key for a security-sensitive query.
- Evicting before a transaction commits.
- Setting no TTL because the current dataset is small.
- Treating `DEL cache:*` or `KEYS cache:*` as normal invalidation.
