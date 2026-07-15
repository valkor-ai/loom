# Nuxt Application Boundaries

Apply this reference only for an accepted Nuxt stack when the task owns Nuxt routing, server-rendered data/composition, server mutations/routes, runtime configuration, or framework migration. Ordinary Vue components use Vue references without loading the whole Nuxt boundary.

## File And Runtime Ownership

Follow the installed Nuxt version's `pages`, `layouts`, `middleware`, `plugins`, `composables`, `server`, `shared`, `public`, and module conventions. Keep auto-import behavior visible enough that server/client ownership remains understandable.

Confirm SSR/SPA/SSG/hybrid route rules, Nitro preset, deployment runtime, and module compatibility before adopting examples. Do not change hosting/runtime mode for one feature.

Use server-only and client-only module boundaries intentionally. Browser bundles must not import secrets, database clients, filesystem-only code, or privileged provider SDKs.

## Pages, Layouts, And Middleware

Validate dynamic/catch-all/query params before reads/actions. Represent invalid, not-found, forbidden, and unavailable states through Nuxt route/error conventions rather than generic client crashes.

Layouts own coherent product chrome/providers; route middleware owns navigation gating; server handlers still enforce authentication/authorization. Avoid redirect logic duplicated in every page.

Preserve navigation history, return context, metadata, and route rules. Keep product data fetching out of purely presentational layout components.

## Data Fetching

Use `useFetch` for HTTP-aware fetches and `useAsyncData` for arbitrary async data according to repository patterns. Choose server/lazy/immediate/watch/dedupe/default/transform behavior deliberately.

Provide stable unique keys containing every resource, route, locale, identity/tenant, and filter dimension. A process/client payload cache must not reuse one user's result for another.

Avoid duplicate server fetch plus mounted client fetch. Understand payload hydration and when `refresh`/`clear` affects consumers sharing a key.

Parallelize independent data, avoid page-level waterfalls, and retain meaningful loading/error/empty/refreshing state for lazy client updates.

## Hydration And Client Boundaries

Initial server and client markup must be deterministic. Guard browser storage, window size, time/random/locale, third-party widgets, and client-only permissions with stable fallback plus post-hydration update.

Use `<ClientOnly>` only around the smallest browser-only region and provide a fallback that preserves layout. Do not hide broad pages or suppress hydration warnings instead of fixing ownership.

Plugins must declare server/client scope and injected types; per-request state cannot live in a process-global singleton.

## Server Routes And Mutations

Implement server handlers from the accepted interface contract: method/path/input/status/error/auth/exposure. Parse body/query/params, authenticate, authorize the target, validate, call application logic, and map safe errors.

Do not place substantial business/persistence logic in handlers or create a duplicate API beside an existing backend without explicit architecture ownership.

For mutations, enforce CSRF/origin/session/token/idempotency/concurrency policy as applicable and return identity/version/status for UI readback. Never trust client-visible guards.

## Runtime Configuration

Keep secrets in private runtime config and intentionally public values under `runtimeConfig.public`. Validate required values at startup/request boundary and avoid hardcoded local hosts.

Understand build-time versus runtime substitution for the selected Nitro preset/container/serverless/static target. Public runtime config remains visible to browsers.

## Cache And Route Rules

Use route rules, prerendering, ISR/SWR/cache headers, and Nitro storage only for data with explicit freshness and identity isolation. Authenticated/mutable responses must not enter shared caches accidentally.

After mutation, reconcile visible data and invalidate/refresh the exact owned key/route. Broad refreshes hide ownership and increase load.

## Metadata And Errors

Use `useHead`/`useSeoMeta` from validated page data and avoid duplicate global/page tags. Keep private/internal state out of public metadata.

Use Nuxt error/not-found boundaries with user-actionable recovery. Do not render provider stack traces or raw server errors.

## Verification

- Run Nuxt typecheck and production build for changed page/server/config/runtime boundaries.
- Exercise SSR direct request, client navigation, lazy refresh, shared-key behavior, and hydration without mismatch suppression.
- Test invalid/not-found/forbidden params, middleware, server auth/validation/errors, and mutation readback.
- Verify private config/server modules are absent from browser output and runtime values resolve in the deployment preset.
- Check cache/route-rule isolation across identity/tenant and exact post-mutation refresh.

## Delivery Evidence

Name the Nuxt file/runtime owner, data key and lifecycle, server/client boundary, runtime config/cache decision, and SSR/client assertion. Development navigation success does not prove SSR, hydration, secret separation, cache isolation, or deployment-preset behavior.

## Unsafe Defaults

- Nuxt reference loaded for every component in a Nuxt repository.
- Unkeyed or under-keyed async data shared across users/filters.
- Browser-only values changing initial hydration markup.
- Server routes duplicating an accepted backend/application boundary.
- Private config imported into client modules.
- Authenticated/mutable data cached by broad route rules.
- Hosting/Nitro preset changed for a feature-only task.
