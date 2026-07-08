# Nuxt Quality

This file applies Nuxt 3 rules to task-owned pages, layouts, middleware, server routes, composables, plugins, runtime config, metadata, and hydration behavior.

## When To Use

- The task changes Nuxt `pages`, `layouts`, `middleware`, `server/api`, `plugins`, `composables`, `nuxt.config.*`, `useFetch`, `useAsyncData`, `useHead`, `useSeoMeta`, runtime config, or Nitro preset behavior.
- Use this when SSR/SSG, Nuxt file conventions, auto-imports, hydration, server routes, or runtime config affect the feature.
- If the repository is a plain Vue SPA, use Vue core/build references without this Nuxt file.

## Implementation Focus

- Follow Nuxt file conventions and existing module usage. Do not create custom router or server structure when Nuxt conventions already cover the route.
- Use `useFetch`, `useAsyncData`, `useLazyFetch`, or existing API plugins according to the data lifecycle. Keep SSR blocking versus lazy client refresh deliberate.
- Keep server routes thin: parse input, validate, call owned application logic, map status/errors, and avoid duplicating backend API responsibilities unnecessarily.
- Put route protection in route middleware or server-side guards where the protected behavior actually executes.
- Keep server-only runtime config under private runtime config and browser-readable values under `runtimeConfig.public`.
- Use Nuxt plugins for app-level injected services and type them through module augmentation.
- Use `useHead` or `useSeoMeta` for page metadata. Keep public route SEO data aligned with fetched page data.
- Guard client-only browser APIs with `<ClientOnly>`, `onMounted`, or lazy hydration patterns to avoid hydration mismatches.
- Keep Nitro preset, deployment target, and server engine settings aligned with the accepted runtime model; do not change hosting assumptions for a feature-only task.

## Verification Focus

- Run `nuxt build`, `nuxi typecheck`, or the repository's Nuxt build/type command when Nuxt boundaries change.
- Probe SSR data, lazy data, route middleware, server route status/errors, metadata, and hydration-sensitive UI touched by the task.
- Verify missing runtime config fails clearly and public/private runtime config does not leak secrets.
- Test client-only and lazy-hydrated components for fallback and ready states.

## Evidence Focus

- In the evidence summary, name the Nuxt decision: page/layout convention, SSR data strategy, server route, route middleware, runtime config split, plugin injection, metadata, hydration guard, or Nitro/runtime proof.
