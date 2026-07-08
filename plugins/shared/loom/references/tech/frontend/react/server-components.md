# React Server Components Quality

This file applies React Server Component discipline to task-owned server/client boundaries, streaming, serializable props, and Server Component migration.

## When To Use

- The task changes React Server Components, Client Component boundaries, Suspense streaming, server actions in a React framework, or pages that fetch data on the server.
- Use this for Next.js App Router or other RSC-enabled frameworks when the task explicitly owns server/client composition.
- If the repository is a plain client-side React app, do not load this file merely because React is selected.

## Implementation Focus

- Keep components server-side by default only in frameworks where that is the established convention. Add a client boundary only where interactivity, state, effects, refs, or browser APIs are required.
- Do not move data fetching into a Client Component just to make implementation easier when the route convention supports server fetching.
- Pass only serializable data from Server Components to Client Components. Do not pass functions, class instances, database clients, promises that the client cannot own, or secrets.
- Isolate browser-only code behind Client Components and avoid importing server-only modules into client bundles.
- Use Suspense boundaries around slow or independent async regions so streaming does not block the entire page.
- Fetch independent data in parallel where possible; keep sequential fetches only when later requests depend on earlier results.
- Keep server actions or server mutations close to the feature boundary, validate inputs, preserve auth/permission checks, and revalidate or refresh affected UI paths explicitly.
- Avoid hydration mismatches from random values, time-dependent rendering, locale formatting, or viewport-only branching.
- Keep SEO-critical content server-rendered when the framework supports it and current scope requires public discoverability.

## Verification Focus

- Run the framework build that checks server/client boundary errors.
- Verify interactive Client Components still receive the data they need after serialization.
- Test or probe loading, error, and success states for streamed or async regions.
- For server actions, prove success, validation failure, and revalidation/readback behavior when feasible.

## Evidence Focus

- In the evidence summary, name the server/client decision: client boundary placement, serializable props, parallel data fetch, Suspense boundary, server action validation, revalidation, or hydration mismatch prevention.

