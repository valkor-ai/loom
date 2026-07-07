# Next.js Data Fetching Quality

This file applies Next.js data-fetching and caching rules to task-owned pages, server components, route handlers, and data helper modules.

## When To Use

- The task changes `fetch` behavior, cache mode, ISR, revalidation, tags, server-side data helpers, database reads from Server Components, loading/error behavior, or client-side data refresh.
- Use this when freshness, caching, streaming, deduplication, or server/client data ownership affects correctness.
- If the task only changes static markup or local component state, do not introduce new caching or data helpers just because this file exists.

## Implementation Focus

- Make freshness explicit. Choose `cache: 'force-cache'`, `cache: 'no-store'`, `next.revalidate`, segment `revalidate`, or tags according to the business data lifecycle.
- Use tag/path revalidation when a mutation must update already rendered data. Keep tag names stable and domain-owned.
- Fetch independent server data in parallel; keep sequential fetches only when later work depends on earlier results.
- Use React `cache()` or an existing repository helper for repeated server reads during one render pass. Do not create global mutable caches for request-owned data.
- Keep database clients and secrets in Server Components, server helpers, route handlers, or server actions only.
- Use Suspense, `loading.tsx`, or existing skeleton patterns for slow independent regions. Avoid blank screens for async route data.
- Let `error.tsx` or route-level error handling own unexpected fetch failures. Map expected business failures to user-actionable UI states instead of generic exceptions.
- Use client-side data libraries only when the interaction genuinely needs browser-owned refresh, polling, optimistic UI, or stale-while-revalidate behavior.

## Verification Focus

- Run build/typecheck and focused tests for changed data helpers, route handlers, or server components.
- Probe stale/fresh behavior after mutation when revalidation is part of the change.
- Verify loading, empty, success, expected business failure, and unexpected fetch failure states when touched.
- For database reads, verify query shape, auth/tenant scoping, and no accidental client bundle import.

## Evidence Focus

- In the evidence summary, name the data decision: cache mode, ISR interval, tag/path revalidation, parallel fetch, server helper, Suspense boundary, client refresh, or freshness proof.
