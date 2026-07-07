# Next.js Server Components Quality

This file applies Next.js React Server Component rules to task-owned server/client boundaries, streaming, serializable props, and hydration behavior.

## When To Use

- The task changes Server Components, Client Components, `'use client'` placement, Suspense streaming, server-only helpers, context providers, third-party browser components, or hydration-sensitive rendering.
- Use this when Next.js server/client composition affects bundle size, data access, interactivity, or runtime correctness.
- If the project is a plain client-rendered React app, use React references without this Next.js-specific file.

## Implementation Focus

- Keep Server Components as the default for route data, static content, and server-owned business reads.
- Push Client Components down to leaf islands that need event handlers, state, effects, refs, browser APIs, or third-party browser-only widgets.
- Pass only serializable values from Server Components to Client Components. Do not pass functions, class instances, database clients, promises the client cannot own, secrets, or raw ORM entities.
- Wrap third-party browser-only components in a small Client Component adapter rather than marking an entire route as client-side.
- Place providers in a deliberate Client Component wrapper and keep server-rendered route content outside that wrapper when possible.
- Use Suspense boundaries around slow independent server regions so streaming can reveal useful UI early.
- Avoid hydration mismatches from time, randomness, locale-only client formatting, viewport checks, or browser storage reads during server render.
- Keep server-only imports out of client modules. If a helper reads secrets, filesystem, database clients, or `next/headers`, it must not cross a client boundary.

## Verification Focus

- Run `next build` or the repository build to catch server/client boundary and serialization errors.
- Probe interactive islands to confirm they receive correct initial data and preserve accessibility.
- Verify loading, error, and streamed regions for slow server data when touched.
- Inspect bundle or build output only when the task claims bundle-size or client-boundary improvement.

## Evidence Focus

- In the evidence summary, name the server component decision: client island, serializable props, provider boundary, third-party wrapper, Suspense stream, hydration guard, or server-only import proof.
