# Next.js Runtime And Production Build

This reference owns task-scoped Next application configuration, environment validation, runtime selection, middleware/headers/rewrites, image settings, output mode, build/performance instrumentation, and health routes. Deployment assets remain in Loom deploy.

## Environment And Build-Time Semantics

Next environment values may be consumed at build time, server runtime, or browser bundle time. Define when each value is read and validate mandatory server settings before serving the affected capability.

Only deliberate `NEXT_PUBLIC_*` values are exposed to clients and they may be frozen into build output. Never put secrets in public variables or expect a built client bundle to adopt runtime env changes automatically.

Use typed/config schema validation at the server boundary and avoid importing server env modules into Client Components. Keep local defaults runnable and production defaults safe.

## next.config And Output

Preserve the project's config module format and selected Next version. Change `output: 'standalone'` only when self-host/deploy facts require it; deployment packaging must copy standalone server, static assets, and public assets correctly.

Configure `basePath`, asset prefix, trailing slash, output mode, transpilation, experimental flags, and compiler options only from accepted runtime/repository needs. These affect routes/assets/deploy topology globally.

Remote image patterns should allow only actual schemes/hosts/ports/path patterns. Keep image sizes/formats/loader aligned with content and hosting. Do not use wildcard remote images as a convenience.

## Node, Edge, And Static Runtime

Choose Node runtime for Node APIs, database/native drivers, filesystem, and broad library compatibility. Choose Edge only when all dependencies and behavior are compatible and latency/distribution benefit is accepted.

Static export cannot provide Server Actions, dynamic server reads, cookies/headers, route handlers requiring runtime, or ISR in the same way as a server runtime. Do not select it for an incompatible app.

Verify runtime inheritance per segment/handler/middleware and do not mix unsupported modules into Edge bundles.

## Middleware, Rewrites, Redirects, And Headers

Keep matchers narrow and exclude `_next` assets, images, metadata/static files, health, and API routes unless intentionally handled. Middleware must remain lightweight and runtime-compatible.

Rewrites/redirects/proxy paths must preserve accepted public API and frontend route ownership. Avoid catch-all rewrites that turn API requests into HTML or create redirect loops.

Add CSP/HSTS/frame/referrer/permissions/cache/security headers according to hosting/security design. Nonces and third-party scripts require a complete CSP strategy, not copied literals.

## Health And Observability

Health/readiness route handlers should be lightweight, bounded, non-mutating, and expose only necessary status. Separate process liveness from required dependency readiness when the platform uses both.

Use repository logging/instrumentation/analytics conventions and redact cookies, tokens, headers, personal data, and internal provider details. Keep telemetry initialization/runtime compatible and avoid duplicate client/server events.

## Performance And Bundles

Use production build output, bundle analyzer, route sizes, Web Vitals, and representative browser traces when performance is task-owned. Do not impose a universal Lighthouse score copied from an external skill.

Reduce client boundaries/dependencies, parallelize server reads, optimize images/fonts, and split expensive browser libraries based on measured risk. Avoid broad dynamic imports, `ssr: false`, or caching as generic optimization.

Instrument Web Vitals only through accepted telemetry and bounded dimensions. Development mode is not performance evidence.

## Self-Hosting Runtime

Preserve package manager/scripts and required Node version. Understand standalone/public/static copying, proxy trust, host/port, graceful shutdown, connection pooling, file-system persistence, and multi-instance cache/revalidation behavior.

Do not write durable uploads/database files to ephemeral application paths unless runtime delivery declares persistent storage. Avoid assuming in-memory caches/queues/sessions are shared across replicas.

Docker/Compose/proxy generation and route topology remain Loom deploy responsibilities; application config should expose accurate structured facts and runtime-safe behavior.

## Verification

- Run production build for config/env/runtime/middleware/output/image/performance changes.
- Start the built artifact with valid/missing settings and assert clear behavior.
- Probe exact matcher/header/rewrite/redirect/health routes and API exclusions.
- Verify Node/Edge/static compatibility with changed dependencies/features.
- Inspect client/server bundles for public env, secrets, server imports, and measured size claims.
- Verify standalone/self-host static/public assets and runtime binding when owned.

## Delivery Evidence

Identify config/runtime/build decision and production build/start/route/bundle assertion proving it. Dev server success or config text alone cannot prove build-time env behavior, runtime compatibility, matcher topology, standalone packaging, or secret isolation.

## Unsafe Defaults

- Deployment Docker/Vercel instructions duplicated in application references.
- Secrets placed in `NEXT_PUBLIC_*` or server env imported client-side.
- Standalone/Edge/static output enabled without feature/runtime compatibility.
- Catch-all middleware/rewrites intercepting assets or API paths.
- Universal Lighthouse threshold used as a contract.
- Durable files or shared state assumed on ephemeral/multi-instance runtime.
