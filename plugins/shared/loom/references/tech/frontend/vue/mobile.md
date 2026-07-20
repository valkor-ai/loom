# Vue Hybrid, Native Shell, And PWA Delivery

Apply this reference only when the accepted stack includes Quasar, Capacitor, or PWA capability and the task explicitly owns mobile/native/offline/platform behavior. Do not add hybrid infrastructure to an ordinary Vue web task.

## Select The Runtime

Identify whether the deliverable is responsive web, installable PWA, Quasar SPA/PWA/Capacitor, plain Vue plus Capacitor, Electron/BEX, or multiple explicit modes. Each mode has different config, asset, routing, storage, permission, update, and deployment behavior.

Preserve the repository framework/mode. Do not introduce Quasar or Capacitor merely for mobile-looking layout, and do not assume a PWA service worker satisfies native requirements.

Keep shared product components independent of shell APIs where practical; expose native/PWA capabilities through typed adapters/composables.

## Quasar Boundaries

Use boot files for app-level clients/plugins and keep their server/client/mode guards explicit. Register only framework plugins/components needed by the product; blanket imports increase bundles and hide dependencies.

Respect Quasar layout/page container ownership, screen utilities, dark/theme tokens, dialogs/notify/loading lifecycle, and accessible labels. Global loading/notify must close on every outcome and must not replace contextual errors.

Build mode, router mode/base, public path, icon sets, extras, and environment config must match deployment/native packaging.

## Capacitor Configuration

Treat app ID/name, `webDir`, server settings, schemes, plugins, permissions, platform projects, signing, and sync as release-impacting. Never ship a development LAN URL, cleartext override, or live-reload server config in production.

After web build/config/plugin changes, use the repository sync/copy/native build process. A TypeScript import does not prove plugin installation on iOS/Android.

Guard plugin use with runtime/capability checks and model granted, denied, blocked, unavailable, interrupted, and native-error states. Provide a browser fallback or clear unavailable state where web mode is supported.

Remove app/plugin listeners and handle foreground/background, process recreation, and restored navigation/state when owned.

## PWA Manifest And Installation

Keep manifest ID/scope/start URL/display/orientation/theme/icons aligned with router base and deployment path. Use install prompts only when the browser exposes them and the product has a meaningful trigger.

Model install available/dismissed/installed, update available/apply later, offline/reconnected, and unsupported states without blocking ordinary browser use.

Do not claim iOS/Android native parity from PWA installation; platform capabilities and update lifecycles differ.

## Service Worker Caching

Choose strategies per asset/resource: precache versioned app shell, cache-first immutable assets, network-first bounded public data, and network-only sensitive mutations/authenticated responses unless an explicit offline architecture says otherwise.

Never cache login/token responses, personalized mutable APIs, or error responses under a shared key. Include method, URL/query, identity scope, freshness, size/count/expiry, and invalidation dimensions.

Version/migrate offline queues and define idempotency, conflict, retry/backoff, ordering, tombstones, and user/account cleanup. Background sync is not a substitute for server concurrency policy.

Coordinate service-worker activation/update so an old page is not paired silently with incompatible assets/API schema. Provide a controlled reload/apply path.

## Mobile UI And Accessibility

Use UIX responsive/mobile rules for density, safe areas, touch targets, keyboard, viewport units, standalone mode, reduced motion, and offline feedback. Do not fork business workflows solely by user-agent.

Native dialogs, notifications, haptics, share, camera, geolocation, files, and push integrations require permission/capability/privacy-aware behavior and platform evidence.

## Verification

- Run each affected Quasar/PWA/Capacitor build mode and native sync/config boundary.
- Verify production config excludes dev server URLs/cleartext allowances and resolves correct `webDir`/base/scope/assets.
- Exercise unsupported, permission denied/blocked, plugin failure, background/foreground, and listener cleanup.
- Test service-worker install/update/offline/reconnect/cache expiry and ensure auth/mutations/personalized data are not unsafely cached.
- Validate affected native behavior on available platforms and record unavailable-platform risk accurately.

## Delivery Evidence

Name the runtime mode, platform adapter/plugin, manifest/cache/update policy, and actual web/native evidence. A responsive browser page or successful web build does not prove native plugin installation, service-worker safety, offline conflict behavior, or platform parity.

## Unsafe Defaults

- Quasar/Capacitor/PWA added because the task mentions mobile layout.
- Capacitor production config retaining a LAN URL or cleartext mode.
- Native plugin considered installed after package import only.
- Cache-first policy applied to authenticated or mutable API data.
- Offline queue without idempotency/conflict/account cleanup.
- Service-worker update forcing uncontrolled workflow loss.
- Native parity claimed from browser/PWA evidence.
