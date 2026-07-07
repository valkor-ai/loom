# Vue Mobile And PWA Quality

This file applies Vue mobile, hybrid, Quasar, Capacitor, PWA, service worker, and offline behavior rules to task-owned mobile surfaces.

## When To Use

- The task changes Quasar layouts/components, Capacitor integration, native plugins, mobile platform detection, PWA manifest, service worker caching, install prompts, offline state, push notifications, or app lifecycle behavior.
- Use this when the Vue application must behave correctly on native shells, mobile browsers, or offline-capable web installs.
- If the current product is a desktop-only web app, do not add mobile or PWA infrastructure unless the task owns that requirement.

## Implementation Focus

- Follow the repository's chosen mobile framework. Do not add Quasar, Capacitor, or PWA plugins to a plain Vue app unless the accepted scope requires mobile/hybrid delivery.
- Keep platform detection centralized through existing utilities or framework APIs. Avoid scattering user-agent checks through components.
- Guard native plugin calls behind platform checks and permission checks. Camera, geolocation, push notifications, and app lifecycle APIs must fail gracefully on unsupported platforms.
- Keep mobile layout, navigation, drawer, safe-area, and touch-target behavior aligned with UIX mobile references and existing design tokens.
- For PWA caching, choose Workbox/runtime cache strategies per resource type. Do not cache authenticated or mutable API responses with a stale strategy unless the product accepts it.
- Model offline, reconnect, update-available, install-available, permission-denied, and native-error states explicitly when touched.
- Clean up native listeners, service worker listeners, and browser online/offline listeners on unmount.
- Keep app identifiers, icons, splash screens, and signing/build settings in project configuration, not hardcoded inside feature components.

## Verification Focus

- Run the repository's mobile/PWA build or web build target after plugin/config changes.
- Probe unsupported-platform behavior in a browser and native-platform behavior when the tooling is available.
- Verify permission denied, unavailable plugin, offline, reconnect, service worker update, and install prompt states when touched.
- For PWA caching, verify mutable API responses are not served stale in unsafe flows.

## Evidence Focus

- In the evidence summary, name the mobile decision: Quasar layout, Capacitor plugin guard, platform check, permission flow, offline state, service worker cache strategy, PWA update flow, or mobile build proof.
