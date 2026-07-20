# JavaScript Browser API Quality

## When To Use

- The task changes browser fetch logic, DOM integration, storage, workers, service workers, observers, permissions, timers, animation, clipboard, media, or browser-only runtime code.
- Use this when browser lifecycle, user interaction, resource cleanup, accessibility state, or client-side persistence affects correctness.
- If the code runs only in Node or build tooling, use Node references instead.

## Implementation Focus

- Put fetch behavior behind the project's existing client or a small boundary: base URL, headers, credentials, response status handling, JSON parsing, cancellation, and user-facing error mapping should be consistent.
- Use `AbortController` for requests that can be superseded by navigation, unmount, search input changes, or repeated submissions. Ensure the lifecycle owner actually calls abort.
- Clean up event listeners, observers, intervals, timeouts, workers, media streams, and subscriptions in the framework lifecycle or owning module teardown.
- Treat `localStorage` and `sessionStorage` as small, synchronous, failure-prone stores. Guard JSON parsing, handle quota/security failures, and do not store secrets unless the application already has an accepted security model for it.
- Use IndexedDB or an existing client cache for larger or structured offline data, and define version upgrade behavior when schema shape changes.
- Gate permission-based APIs such as clipboard, notifications, camera, microphone, geolocation, and file system access behind a user action and a visible denied/error state.
- Avoid blocking the main thread with heavy parsing, rendering, or computation. Use debouncing, scheduling, workers, or chunked processing when the task can produce noticeable UI stalls.
- In framework apps, do not bypass the framework's state/rendering model with direct DOM mutation unless integrating a library that requires it; isolate such integration behind a component or adapter.
- Keep browser feature use compatible with the configured targets or include the existing polyfill/transpile path.

### Worker And Observer Ownership

Workers, service workers, observers, media streams, and event listeners need an explicit owner. Validate message shape at the boundary, report worker errors, terminate workers when the feature is no longer used, and disconnect observers during teardown. A callback that captures a component or DOM subtree must not outlive that owner.

### Storage And Cache Evolution

Treat browser storage as an unreliable boundary: parsing, quota, private-mode/security errors, stale schema, and unavailable APIs are expected states. Version IndexedDB upgrades transactionally and define how old records are migrated or discarded. Service-worker cache names need versioning, invalidation, install/activate failure handling, and an explicit network/cache policy; a cached success must not hide a newer error or authorization change.

### Permission And Main-Thread Boundaries

Request permission from a user action, preserve denied and dismissed states, and avoid repeated prompts. Move CPU-heavy parsing or transformation to a worker or bounded chunks when it can block interaction. Verify that the chosen browser API and fallback path match the repository's target browsers rather than assuming a modern API exists everywhere.

## Verification Focus

- Run browser-oriented tests or a framework build for changed browser code.
- Test success, network failure, non-2xx response, abort/unmount, and malformed response paths when fetch logic changed.
- Verify cleanup for observers, workers, timers, and event listeners when lifecycle code changed.
- For UI-facing browser behavior, smoke the relevant viewport or interaction path rather than relying only on unit tests.
- For workers, storage, service workers, permissions, and observers, verify unavailable/denied, malformed, upgrade, teardown, and stale-result paths as applicable.

## Evidence Focus

- In the evidence summary, name the browser decision: fetch boundary, cancellation, lifecycle cleanup, storage strategy, permission flow, main-thread protection, DOM integration, or target compatibility.
