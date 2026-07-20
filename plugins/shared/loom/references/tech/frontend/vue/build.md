# Vue Build And Runtime Configuration

Apply build guidance only when the task owns Vite/Vue plugin setup, aliases, environment/config, dev proxy, code splitting, assets, sourcemaps, PWA integration, bundle performance, or framework migration.

## Preserve The Toolchain

Inspect package manager, scripts, Vite/Rollup version, Vue plugin/compiler, monorepo root, aliases, CSS preprocessors, test transforms, environment modes, and deploy output before editing configuration.

Do not replace an established Vue CLI, Nuxt, Quasar, library build, or custom bundler with plain Vite as an incidental change. Nuxt/Quasar own their wrapper configuration where applicable.

Keep configuration deterministic and avoid executing network calls or environment-sensitive filesystem discovery during build unless the repository intentionally does so.

## Plugins And Generated Imports

Add a plugin only for task-owned behavior and confirm version compatibility, plugin order, server/build/test behavior, and generated-file ownership.

Auto-import/component plugins need scoped directories/resolvers, deterministic declaration output, lint/type integration, and collision policy. Do not enable repository-wide magic imports for one component.

Dev-only inspection plugins must not expose production routes/data or inflate production bundles.

## Aliases And Monorepos

Resolve aliases through path-safe URL/filesystem APIs and keep Vite, TypeScript/JavaScript, tests, lint, SSR, and workspace package exports aligned.

An editor-resolved alias can still fail in tests or production. Avoid aliases that bypass package public exports or create duplicate Vue/runtime copies.

Configure monorepo filesystem access, dependency optimization, symlinks, and watch roots narrowly; do not expose the whole workstation through the dev server.

## Environment And Public Config

Treat `VITE_*` and every client-injected value as public. Secrets, signing keys, provider credentials, and private backend endpoints must remain server/deploy configuration.

Type and validate required public values. Distinguish missing, empty, invalid URL/path, and mode-specific values; do not silently fall back to localhost in production.

Preserve same-origin/API base-path contracts and deployment subpaths. Build `base`, router history base, asset URLs, and service-worker scope must agree.

## Development Proxy

Proxy only accepted path ownership and preserve method/path/query/body/headers/cookies/streaming/WebSocket behavior required by the API.

Do not strip `/api` or rewrite routes merely to make development pass when production preserves the path. Avoid proxy configuration becoming a second undocumented API contract.

Keep proxy targets environment-owned and prevent an externally reachable dev server from becoming an open proxy.

## Code Splitting And Chunks

Use route dynamic imports or `defineAsyncComponent` for heavy optional regions with stable loading/error/retry UI. Confirm generated chunks through a production build.

Manual chunks require measured cache/bundle benefit and must avoid circular or one-package-per-chunk explosions. Preserve CSS/assets and predictable invalidation.

Keep server-only/optional dependencies out of client chunks and avoid namespace imports or package entry points known to defeat tree shaking.

## Assets, CSS, And Sourcemaps

Handle public versus imported assets, base paths, hashed output, fonts, worker URLs, and CSS side effects according to Vite semantics. Referenced assets must exist with correct case.

Choose sourcemap mode with error-reporting access policy. Hidden maps still contain source and must be uploaded/stored securely and removed from public artifacts where required.

Do not define build time or random content into deterministic bundles unless reproducibility/cache behavior accepts it.

## Performance And Output

Measure representative route chunks and dependency duplication before optimization. Compression plugins do not replace server/CDN content negotiation and should not produce unused artifacts.

For library builds, preserve external/peer dependency, declaration, CSS, exports, module format, and consumer compatibility contracts.

## Verification

- Run the exact production build plus focused type/lint/test targets after config changes.
- Verify aliases/generated declarations in editor-independent typecheck, tests, and build.
- Probe dev proxy method/path/cookie/error/WebSocket behavior without changing accepted API paths.
- Exercise lazy loading/error and inspect output chunks/assets/base paths/sourcemap publication.
- Test required public config failure and production runtime/deploy substitution.

## Delivery Evidence

Name the config owner, plugin/alias/env/proxy/chunk decision, expected output, and command/artifact proving it. Dev-server success alone does not establish production paths, runtime config, chunk integrity, sourcemap safety, or deploy compatibility.

## Unsafe Defaults

- Plain Vite configuration imposed on Nuxt/Quasar/legacy tooling.
- Plugin or auto-import enabled globally for one local need.
- `VITE_*` treated as secret storage.
- Development proxy rewriting away the production API contract.
- Manual chunks copied without bundle measurement.
- Hidden sourcemaps shipped publicly.
- Alias updated in only one resolver.
