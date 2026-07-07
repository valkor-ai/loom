# Next.js Runtime Quality

This file applies Next.js runtime and production-readiness rules to task-owned configuration, environment variables, standalone output, headers, health routes, and runtime selection.

## When To Use

- The task changes `next.config.*`, image remote patterns, standalone output, environment variables, public/private runtime config, headers, rewrites, middleware runtime, health endpoints, or build/deploy readiness.
- Use this for application runtime behavior. Container, Compose, hosting-provider, and deployment asset generation belong to Loom deploy references.
- If the task only changes a component with no runtime behavior, do not load this file.

## Implementation Focus

- Keep environment variables explicit and typed where the repository supports validation. Server-only values stay server-only; browser values use deliberate public prefixes.
- Use `output: 'standalone'` only when self-hosting or deploy tooling requires it. Do not change output mode for unrelated feature work.
- Configure image `remotePatterns`, formats, and sizes only for actual image sources used by the product.
- Add security headers, redirects, rewrites, and middleware narrowly. Avoid global catch-all behavior that hides API or route ownership.
- Choose Node.js or Edge runtime based on dependencies. Do not use Edge runtime for code that needs Node APIs, database drivers, filesystem, or unsupported native modules.
- Keep health/readiness route handlers lightweight and free of business workflows. They may check required dependencies but should not mutate data.
- Keep analytics and performance instrumentation behind existing project conventions and environment controls.
- Preserve existing package manager and script names. Do not invent new release commands when `next build`, `next start`, or framework scripts already exist.

## Verification Focus

- Run `next build` after config, runtime, middleware, image, header, or environment changes.
- Probe health/runtime route handlers when changed.
- Verify missing or invalid required environment values fail with a clear message before production traffic.
- For Edge runtime or middleware, test the exact route paths and header/redirect behavior touched by the task.

## Evidence Focus

- In the evidence summary, name the runtime decision: environment split, standalone output, image config, security header, rewrite/redirect, Edge/Node runtime, health route, or production build proof.
