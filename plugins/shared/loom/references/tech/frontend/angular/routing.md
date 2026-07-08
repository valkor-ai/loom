# Angular Routing Quality

This file applies Angular Router discipline to task-owned route trees, lazy feature routes, route guards, resolvers, route params, outlets, and navigation behavior.

## When To Use

- The task creates or changes Angular route configuration, feature route files, lazy-loaded screens, guards, resolvers, route params, query params, redirects, or navigation state.
- Use this for route ownership, route-level data loading, auth/business access checks, deep links, title metadata, and not-found behavior.
- If the task only changes a component used inside an unchanged route, keep this file out of scope unless navigation behavior or route data changes.

## Implementation Focus

- Keep each route segment mapped to a clear product surface. Do not place unrelated workflows behind one catch-all route or a component that switches manually on path text.
- Prefer lazy route boundaries for substantial feature areas, admin sections, or mobile/web surfaces that are not needed on initial load.
- Use functional guards and resolvers with `inject()` in compatible Angular versions. Keep class guards only when existing code or version constraints require them.
- Use guards for access decisions and resolvers for route-critical data needed before rendering. Do not hide route access failures as generic component loading failures.
- Bind route params to component inputs when the application uses `withComponentInputBinding()`. Otherwise subscribe with cleanup or convert route observables safely.
- Validate route params before API calls, store dispatches, or mutation commands. Missing, malformed, or unauthorized IDs need explicit route-level outcomes.
- Preserve query params intentionally when filters, tabs, pagination, or return URLs need deep-link behavior. Avoid accidental query param loss during programmatic navigation.
- Add not-found, unauthorized, and blocked states where route data can fail for domain reasons. Do not redirect all failures to the same home page.
- Use preloading deliberately for high-value follow-on feature routes. Do not preload heavy admin/mobile/reporting routes without a product reason.
- Keep router event subscriptions cleaned up with `takeUntilDestroyed()` or an equivalent repository pattern.

## Verification Focus

- Probe the changed route paths, dynamic params, query params, redirects, guard success/failure, resolver success/failure, and not-found states.
- Verify lazy route imports compile and do not introduce circular dependencies or missing standalone imports.
- Verify browser refresh/deep-link entry for any route that must be shareable or directly reachable.
- Verify navigation preserves or clears query params according to the workflow requirement.
- Run Angular build and route-focused tests for guards, resolvers, and navigation helpers when available.

## Evidence Focus

- In the evidence summary, name the route decision: lazy feature boundary, functional guard, resolver, param validation, query preservation, not-found handling, redirect rule, or deep-link proof.
