# Next.js Data Fetching And Freshness

Apply data guidance when the task owns an API/server data binding, client reactive flow, or accepted full-stack persistence read. Choose freshness, cache, streaming, and client ownership from business lifecycle rather than Next.js defaults or external examples.

## Data Ownership

Preserve the accepted boundary:

- call the authoritative backend interface when architecture separates frontend/backend
- use server-only repository/data helpers only when full-stack Next owns persistence
- use client data libraries only for browser-owned refresh/polling/optimistic workflows
- never duplicate the same source of truth across server fetch, client effect, and store without reconciliation

Enforce auth/tenant/ownership at the server/data boundary. Do not pass database clients/secrets/entities into Client Components.

## Freshness And Cache Policy

Make each read's policy explicit: dynamic/no-store, cached/static, time revalidation, tag/path invalidation, or client stale-while-revalidate.

Use the selected Next version's fetch/cache APIs; semantics changed across versions. Do not assume implicit fetch caching. Define cache key dimensions, user/tenant variation, invalidation triggers, stale tolerance, and failure behavior.

Never share personalized/authorized data through a cache key that omits identity/tenant/permissions. `revalidateTag`/path invalidation should be domain-owned and scoped.

React `cache()`/request memoization can deduplicate server work in a render/request boundary; it is not a durable cross-request cache unless the selected framework API explicitly provides that.

## Parallel, Sequential, And Preloaded Reads

Start independent reads before awaiting to avoid server waterfalls:

```tsx
const orderPromise = loadOrder(orderId)
const historyPromise = loadHistory(orderId)
const [order, history] = await Promise.all([orderPromise, historyPromise])
```

Keep sequential calls only when later inputs/security depend on earlier results. Bound fan-out and handle partial failure when one region may remain useful.

Preload/deduplicate only when the user flow is likely and cache/scoping is safe. Do not fire duplicate page/layout/metadata/client requests for the same record.

## Streaming And Loading

Use route loading or Suspense around independently useful slow regions with stable fallback layout. Keep page shell/context/actions visible and avoid a blank full-page wait.

Expected empty/not-found/forbidden/unavailable outcomes should map to product states rather than generic exceptions. Unexpected failures reach the owning error boundary.

Do not stream protected record details before authorization resolves.

## Direct Database Reads

Only server modules may use accepted database/ORM clients. Reuse connection pools safely for the runtime (Node server, serverless, edge limitations) and project directly to serializable read models.

Avoid N+1/unbounded queries, full entity graphs, and database calls from Client Components. Provider mapping/query/transaction quality remains owned by the selected backend/persistence references.

## Client-Side Data

Use client fetching when interactions require live browser refresh, polling, infinite scroll, optimistic state, or browser-only context. Seed with safe server data when it improves initial render and define hydration/freshness reconciliation.

Cancel/ignore stale requests, bound polling/concurrency, preserve typed errors, and invalidate after mutation/user/tenant changes. A `useEffect(fetch)` without race/error/loading cleanup is not a complete data boundary.

## Pagination, Filtering, And Serialization

Forward accepted query parameters and bound page size/sort/filter allowlists. Preserve deterministic order and stable response metadata.

Normalize dates, decimal/bigint, enums, nullable fields, and errors before client handoff. Do not leak internal/provider fields in serialized props or route responses.

## Revalidation After Mutation

Mutations must invalidate all and only affected server/client cache entries and reconcile visible list/detail/count/state. Readback evidence should prove new identity/version/status rather than assuming invalidation worked.

Avoid invalidating the whole site or using tags that collide across tenants/resources. Define behavior when revalidation succeeds but client state still contains optimistic/stale data.

## Verification

- Test exact authoritative API/repository binding and auth/tenant scoping.
- Prove cache hit/freshness/stale behavior and no cross-user/tenant leakage where caching is owned.
- Verify mutation invalidation and visible readback for list/detail/count.
- Exercise parallel/sequential/partial failure, loading/empty/forbidden/unavailable states.
- Test client race/cancellation/polling/infinite-scroll behavior when owned.
- Run production build for server/client and runtime compatibility.

## Delivery Evidence

Identify source of truth, freshness policy, cache key/invalidation, and route/component assertion proving it. A fetch call, Suspense fallback, or revalidation invocation alone cannot prove scoping, freshness, race handling, serialization, or visible coherence.

## Unsafe Defaults

- Data reference selected from prose/performance without data-binding ownership.
- Implicit cache behavior assumed across Next versions.
- Personalized data cached without identity/tenant keying.
- Same read duplicated in layout/page/metadata/client effect.
- Client effect fetches without cancellation/error/reconciliation.
- Whole-site invalidation or direct DB access from client modules.
