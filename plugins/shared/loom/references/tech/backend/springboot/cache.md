# Spring Boot Cache Implementation

Spring Cache is an optimization around an owned source of truth. It must not become an implicit second source of truth or change authorization and business semantics.

## Cache Contract

Define before adding annotations:

- source of truth
- cache name and owner
- key shape and tenant/actor scope
- cached value shape
- freshness/TTL expectation
- mutation invalidation or update behavior
- null, not-found, and error caching policy
- provider-unavailable behavior
- sensitive-data restrictions

No cache is required merely because Spring Boot supports one.

## Key Design

Prefer explicit, stable keys over default argument serialization.

```java
@Cacheable(cacheNames = "order-summary", key = "#tenantId + ':' + #orderId", sync = true)
public OrderSummary findSummary(String tenantId, UUID orderId) {
    return queries.findSummary(tenantId, orderId).orElseThrow(OrderNotFound::new);
}
```

Include every dimension that changes authorization or result content. Do not share entries across tenants, locales, permission scopes, or query filters unintentionally. Avoid mutable objects and JPA entities as cached values; use immutable DTOs/read models.

## Invalidation And Transaction Timing

Mutation and cache timing must agree:

- evict/update only after a successful commit when cached data reflects database state
- invalidate all keys affected by a mutation, including list/query caches
- define behavior for bulk updates and external writers
- avoid `allEntries = true` unless the cache is small and globally invalidated by design

Spring proxy rules apply to cache annotations. Same-class self-invocation bypasses caching. Annotation ordering with transactions can expose uncommitted or rolled-back data if eviction/update happens at the wrong boundary.

Prefer simple cache-aside behavior. `@CachePut` is useful only when the returned value exactly represents committed cache state. Do not combine `@Cacheable` and `@CachePut` on the same method without a proven condition model.

## Freshness And Provider Behavior

Configure TTL and capacity through the selected provider and typed runtime properties. Do not hardcode provider-specific settings into business code.

For distributed caches, define serialization compatibility, key namespace/versioning, network timeout, and behavior during partial outage. For local caches, define per-instance staleness and memory bounds.

`sync = true` can reduce same-key stampedes within one cache manager; it is not a distributed lock. Expensive or high-contention loads may require provider-specific request coalescing or a different read model.

Do not cache exceptions by accident. Decide whether a not-found result is cacheable and for how long; negative caching can hide newly created data.

## Security And Privacy

Never cache raw credentials, bearer tokens, or unrestricted entities containing sensitive fields. Authorization must run before returning a cached value unless the cache key and cached object are explicitly scoped to the authorized principal/tenant.

## Verification Focus

Useful cache evidence includes:

- miss then hit behavior with the same key
- separation of tenant/actor/filter keys
- invalidation or update after a committed mutation
- rollback leaves the prior cache state intact
- TTL/freshness behavior where owned
- null/not-found/error policy
- concurrent same-key loading behavior
- provider-unavailable fallback without corrupting the source of truth

## Unsafe Defaults

- `@Cacheable` with an implicit key on a security-sensitive query.
- Caching JPA entities or mutable collections.
- Evicting before a transaction commits.
- Global cache clears after every write.
- Introducing Redis or Caffeine without a selected provider/runtime dependency.
- Treating cache availability as a prerequisite when the accepted design says it is optional.
