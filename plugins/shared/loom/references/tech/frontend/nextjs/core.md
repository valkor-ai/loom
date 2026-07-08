# Next.js Core Quality

This file applies Next.js implementation discipline to task-owned App Router applications, route surfaces, full-stack UI features, and production runtime boundaries.

## When To Use

- The task creates or changes a Next.js application, page, layout, route handler, middleware, metadata, image/font setup, or full-stack UI feature.
- Use this for Next.js-specific rendering strategy, server/client composition, route ownership, SEO metadata, asset handling, and build readiness.
- If the task only edits a plain React component outside a Next.js project, use React references without this Next.js reference.

## Implementation Focus

- Follow the repository's existing App Router structure, route groups, aliases, styling system, data access boundary, and environment-variable style before introducing new folders or conventions.
- Prefer App Router for new work. Do not add Pages Router files unless the existing repository is already Pages Router and the task explicitly preserves it.
- Keep components server-side by default when the route supports it. Add `'use client'` only for the smallest interactive boundary that needs state, effects, browser APIs, or event handlers.
- Keep route/page components as orchestration. Extract feature components, server data helpers, client islands, route actions, validation helpers, and formatters when the page becomes hard to inspect.
- Use `metadata`, `generateMetadata`, or existing SEO helpers for page metadata. Do not hardcode `<title>` or ad hoc meta tags inside route JSX.
- Use `next/image` and `next/font` according to repository conventions for real content images and app fonts. Configure remote image patterns when external image hosts are required.
- Keep server-only values on the server. Only expose browser-readable variables through deliberate public environment names such as `NEXT_PUBLIC_*`.
- Keep product UI free of delivery progress, framework explanations, runtime commands, and verification notes.

## Verification Focus

- Run `next build` or the repository's build command when route boundaries, server/client composition, metadata, images, config, or runtime behavior changes.
- Run typecheck, lint, and focused component/route tests when available.
- Probe loading, error, not-found, ready, validation, and business-blocking states touched by the task.
- Verify no server-only module, secret, database client, or filesystem dependency is imported into a Client Component bundle.

## Evidence Focus

- In the evidence summary, name the Next.js decision: App Router structure, server/client boundary, metadata source, image/font configuration, environment split, route handler boundary, or production build proof.
