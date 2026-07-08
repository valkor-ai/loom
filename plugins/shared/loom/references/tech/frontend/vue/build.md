# Vue Build Tooling Quality

This file applies Vue build and Vite rules to task-owned configuration, aliases, environment variables, dev proxies, code splitting, sourcemaps, and bundle optimization.

## When To Use

- The task changes `vite.config.*`, Vue plugin setup, aliases, auto-imports, env variables, dev server proxy, sourcemaps, chunking, bundle analysis, PWA/build plugins, or production build behavior.
- Use this when build configuration affects local development, deploy readiness, debugging, performance, or framework integration.
- If the task only changes a component and the build already works, do not edit Vite config just because this file exists.

## Implementation Focus

- Preserve the repository's package manager, script names, alias style, plugin order, and existing Vite/Vue integration.
- Add Vite plugins only for current behavior: Vue SFC support, DevTools, component auto-import, API proxy, PWA, compression, image optimization, or bundle analysis.
- Keep environment variables under the framework's public prefix rules. For Vite, browser-readable variables use `VITE_*`; secrets must not enter client code.
- Type environment variables through `env.d.ts` or existing project types when code depends on them.
- Keep dev server proxy rules narrow and explicit. Do not rewrite a broad `/api` prefix in a way that hides real backend route ownership.
- Use route-level dynamic imports or `defineAsyncComponent` for heavy optional areas. Add loading and error states for user-visible lazy components.
- Use manual chunks only when a real bundle problem exists. Do not over-split small apps into unstable chunks.
- Configure sourcemaps according to debugging and production-error needs. Hidden production sourcemaps should not be publicly served by accident.
- Use bundle analyzer or web-vitals instrumentation only when the repository already has it or the task owns performance work.

## Verification Focus

- Run the repository build, typecheck, and lint after config, env, alias, or plugin changes.
- Probe dev proxy behavior when routes or API base URLs change.
- Verify lazy-loaded areas render loading, error, and ready states.
- Check bundle or sourcemap output when optimization is claimed.

## Evidence Focus

- In the evidence summary, name the build decision: plugin setup, alias, env typing, dev proxy, lazy loading, manual chunk, sourcemap mode, analyzer, or production build proof.
