# Next.js App Router Quality

This file applies App Router structure rules to task-owned route segments, layouts, pages, route groups, dynamic routes, loading states, and error boundaries.

## When To Use

- The task changes `app/` route files, `layout.tsx`, `page.tsx`, `template.tsx`, `loading.tsx`, `error.tsx`, `not-found.tsx`, route groups, dynamic segments, parallel routes, intercepting routes, or route handlers.
- Use this when route organization, segment state, navigation behavior, metadata, or boundary files affect the delivered workflow.
- If the task only changes an internal component used by a stable route, keep this file out of scope unless route behavior also changes.

## Implementation Focus

- Keep each route segment responsible for a clear product surface. Use route groups for organization without URL changes, not to hide unrelated workflows in one segment.
- Put persistent shell UI in `layout.tsx`; use `template.tsx` only when remount-on-navigation behavior is actually needed.
- Add `loading.tsx` for async route segments where the user otherwise sees a blank wait. Add `error.tsx` where the route can fail independently and recover with reset behavior.
- Add `not-found.tsx` or call `notFound()` when missing domain records have a first-class not-found state.
- Keep dynamic params typed and validated before using them in queries, actions, or route handlers.
- Use `redirect()` for server-known navigation decisions such as missing auth or completed mutations; do not bounce through fragile client-only effects.
- Keep route handlers thin: parse request input, validate, call owned application logic, map status/response shape, and avoid duplicating API design already owned elsewhere.
- Keep Metadata API usage close to the route data needed for SEO. Do not fetch the same domain record twice when the repository has an accepted shared helper.

## Verification Focus

- Run build/typecheck to catch App Router file-contract and server/client boundary errors.
- Probe route navigation, dynamic params, loading fallback, error reset, not-found behavior, and redirects touched by the task.
- For route handlers, prove success, validation failure, not found, conflict, and auth denial when those outcomes are in scope.
- Verify generated metadata for changed public/detail routes when SEO is part of the route surface.

## Evidence Focus

- In the evidence summary, name the route decision: layout ownership, route group, dynamic param validation, loading boundary, error boundary, not-found handling, redirect, metadata, or route handler proof.
