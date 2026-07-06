# JavaScript Browser API Quality

Use this topic reference when `tech/code/javascript/browser.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

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

## Verification Focus

- Run browser-oriented tests or a framework build for changed browser code.
- Test success, network failure, non-2xx response, abort/unmount, and malformed response paths when fetch logic changed.
- Verify cleanup for observers, workers, timers, and event listeners when lifecycle code changed.
- For UI-facing browser behavior, smoke the relevant viewport or interaction path rather than relying only on unit tests.

## Evidence Notes

- Record `javascript.browser` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/javascript/browser.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the browser decision: fetch boundary, cancellation, lifecycle cleanup, storage strategy, permission flow, main-thread protection, DOM integration, or target compatibility.
