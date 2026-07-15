# Next.js React Server Components

Apply this reference only to an App Router task that owns server-rendered component composition, server/client boundaries, streaming, hydration, or serializable handoff. Do not attach it merely because Next.js is selected.

## Boundary Selection

Use Server Components for server-owned reads, static/derived markup, secret/internal access, and reduced browser bundle. Use Client Components for event handlers, local state/effects, refs, browser APIs, client stores, and browser-only libraries.

Push `'use client'` down to the smallest cohesive island. A Client Component boundary makes its import subtree client-side, so do not import server modules beneath it.

```tsx
// Server Component
export async function OrderPanel({ orderId }: { orderId: string }) {
  const order = await loadAuthorizedOrder(orderId)
  return <OrderActions initialOrder={toSerializableOrder(order)} />
}

// Client island
'use client'
export function OrderActions({ initialOrder }: { initialOrder: OrderViewModel }) {
  // interactive state and events only
}
```

## Serializable Handoff

Pass JSON/React-serializable safe values. Do not pass functions, class/ORM instances, database clients, Request/Response objects, secrets, non-serialized Decimal/BigInt, or mutable server handles.

Normalize dates, decimals, bigint, enums, URLs, maps/sets, and errors according to the accepted client contract. Keep payloads minimal and avoid sending hidden/internal fields because the client component does not display them.

Use Server Actions only through their supported reference/form invocation semantics; they are not arbitrary callback props.

## Data And Composition

Fetch at the deepest shared server boundary that avoids waterfalls/duplication while preserving ownership. Parallelize independent reads and use request-scoped memoization/cache semantics deliberately.

Do not convert a component client-side just to fetch data. When browser-owned refresh/polling/optimistic behavior is required, seed a client data boundary with safe initial state and define freshness/reconciliation.

Compose Server Component children through Client Components when the client wrapper does not need to inspect/clone them. Providers require a client wrapper; keep it as narrow as possible and do not turn the root layout into a broad client boundary.

## Streaming And Suspense

Use Suspense around independently useful slow server regions, with stable fallback dimensions and nearby error ownership. Avoid one giant boundary that blocks the whole workbench and avoid dozens of flickering micro-boundaries.

Streaming must preserve authentication/data scoping and not reveal private shell/metadata before access is known. Measure whether early content improves the user flow.

## Browser-Only Libraries

Wrap browser-only third-party components in a focused Client Component adapter and lazy-load when appropriate. Ensure SSR-disabled output has meaningful loading/fallback and does not cause layout shift or blank primary work.

Do not use `dynamic(..., { ssr: false })` as a generic hydration fix. Correct deterministic server/client output or isolate the actual browser dependency.

## Hydration Determinism

Avoid time/randomness, browser storage, viewport checks, locale/timezone differences, generated IDs, and unstable object ordering during server/client initial render.

Pass a server-derived stable value, defer browser-only reads to an effect with a stable placeholder, or use CSS/responsive rendering where possible. `suppressHydrationWarning` is a narrow escape hatch, not a solution for mismatched subtrees.

## Security And Failures

Server Components are reachable through framework requests and must enforce auth/tenant/ownership where data is read. Client hiding is not protection.

Map expected missing/forbidden/business/unavailable outcomes to accepted route/surface states. Unexpected failures reach segment boundaries and correlation-aware logging without leaking internals.

## Verification

- Run production build to catch server/client import and serialization failures.
- Inspect that client bundles exclude server-only modules/secrets/providers.
- Exercise initial data handoff and interactive island behavior.
- Test date/decimal/bigint/error serialization where changed.
- Verify streaming fallback, useful early content, error, and auth scoping.
- Reproduce hydration-sensitive locale/time/storage/viewport paths in a real browser when owned.

## Delivery Evidence

Identify the server/client boundary, payload, and build/browser assertion proving bundle isolation, serialization, streaming, or hydration. A `'use client'` marker or successful dev render cannot prove production import graphs, secret isolation, deterministic hydration, or runtime compatibility.

## Unsafe Defaults

- Server Component reference selected without App Router and explicit boundary ownership.
- Whole pages/layouts converted to Client Components for one hook/widget.
- ORM/classes/secrets/non-serializable provider values passed to clients.
- `ssr: false` or hydration suppression used as a generic repair.
- Root-wide client providers enclosing all server content.
- Streaming boundaries that reveal or displace the whole product shell.
