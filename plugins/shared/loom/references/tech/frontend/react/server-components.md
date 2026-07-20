# React Server Components

Apply this guidance only in a repository whose accepted framework/runtime supports React Server Components and when the task owns a server/client composition boundary. A React dependency alone does not imply RSC support.

## Boundary Design

Keep data access, secrets, server-only dependencies, and non-interactive composition on the server side. Introduce a Client Component at the smallest boundary that needs state, effects, browser APIs, event handlers, or a client-only library.

`'use client'` marks a module boundary and pulls its client import graph into the browser bundle. Do not place it on a page/layout/root merely to make one nested control interactive.

Use a server-only guard supported by the framework for modules that must never enter a client graph. Treat imported environment/config data as public once it crosses a client boundary.

## Serializable Handoff

Pass serializable, minimal view data to Client Components. Convert database models, dates, decimals, maps/sets, class instances, ORM proxies, and provider objects into explicit transport/view models as required by the framework serializer.

Do not pass server functions to clients except through the framework's explicit server-action mechanism. Never serialize secrets, authorization internals, connection objects, or full records when the client needs only identity and display fields.

Preserve stable target identity and version/concurrency fields needed by client actions. Revalidate authorization server-side for every mutation; serialized permission hints control presentation, not access.

## Server Data Access

Fetch at the closest server owner and parallelize independent reads. Avoid sequential waterfalls caused by awaiting unrelated data before constructing child work.

Use framework request memoization and data caching deliberately. Include tenant, user, locale, permissions, filters, and other isolation dimensions in cached data ownership; do not place request-specific data in process-global caches.

Keep HTTP/API access when it is the accepted architecture boundary. Do not replace an existing service contract with direct database access merely because a Server Component can access the server runtime.

## Streaming And Suspense

Place Suspense around an independently slow region with a fallback matching its final dimensions and information hierarchy. Keep critical navigation, titles, and primary actions available when possible.

Pair rejected server work with the framework's error boundary/recovery route. A loading fallback alone does not handle authorization, not-found, or service failure states.

Avoid many tiny boundaries that flash independently or reorder the page incoherently. Streaming order should support the workflow rather than expose implementation timing.

## Client Composition

Prefer passing server-rendered content as `children`/slots into a focused Client Component instead of converting the whole subtree to client code. Keep providers as deep as practical and scope them to consumers.

Client state must not assume that a server-rendered parent will update without navigation, refresh, cache invalidation, or returned action state. Define the readback path after mutation.

Hydration output must be deterministic. Do not branch initial markup on `window`, current time, random values, browser storage, or locale differences without a stable server snapshot and explicit post-hydration update.

## Mutations

Use the framework's server action only when it is part of the accepted interface/runtime contract. Validate input, authenticate, authorize the target, enforce concurrency/idempotency policy, and return actionable state.

After success, reconcile returned data and invalidate/revalidate only affected cache/route ownership. Broad global invalidation hides ownership and increases load.

Keep server-action details in the framework-specific reference when Next.js or another framework defines the transport and deployment behavior.

## Runtime Constraints

Confirm whether the component executes in Node, edge, worker, or another runtime. Filesystem, native packages, sockets, database drivers, crypto APIs, and environment access differ by runtime and deployment target.

Do not rely on development-only co-location. The production build must include required server modules while excluding them from browser chunks.

## Verification

- Run the framework production build and inspect server/client boundary errors.
- Assert server-only modules and secrets are absent from client bundles.
- Exercise serializable handoff for real dates/decimals/nullable/provider-backed values.
- Test loading, not-found, forbidden, service failure, and retry/recovery at owned boundaries.
- Verify mutation authorization, validation, concurrency, readback, and targeted cache invalidation.
- Check hydration without mismatch suppression and test the production runtime target.

## Delivery Evidence

Identify each server/client boundary, serialized handoff, runtime, cache/isolation key, streaming/error boundary, and mutation readback assertion. A page rendering in development does not prove bundle separation, serialization, hydration, or production runtime compatibility.

## Unsafe Defaults

- Treating every React project as RSC-capable.
- Root-level `'use client'` added for a nested interaction.
- ORM/domain/provider objects passed directly across the boundary.
- Direct database access replacing an accepted service/API architecture.
- User-specific results cached without identity dimensions.
- Hydration mismatch hidden with suppression instead of deterministic markup.
- Mutation success without targeted readback or cache reconciliation.
