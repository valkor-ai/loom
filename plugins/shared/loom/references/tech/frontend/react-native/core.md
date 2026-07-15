# React Native Screen And Component Delivery

Implement task-owned mobile workflows within the repository's Expo or bare React Native runtime, renderer version, navigation, styling system, state/data libraries, native-module policy, and mobile UI contract. Do not transfer DOM/CSS/browser patterns into native components.

## Runtime Boundary

Confirm whether the app uses Expo managed/prebuild, bare React Native, Expo Router, React Navigation, the new architecture, Hermes, and platform-specific native projects. Keep dependencies and APIs compatible with those accepted versions.

Use the repository package manager and framework-aware installer where required. Adding a JavaScript import is not enough when a library needs an Expo config plugin, native linking, CocoaPods/Gradle work, permissions, or a development-client rebuild.

Keep secrets and privileged policy out of the bundle. Native applications are distributed client code; hidden screens, local flags, and device storage do not enforce authorization.

## Screen And Component Ownership

Screens orchestrate route params, task-owned data/state, product states, and navigation intent. Reusable components receive typed values and emit commands with stable target identity; they should not hide routing, storage, API, permissions, or analytics work.

Split at independent workflow, state, platform, or reuse boundaries. Avoid monolithic screens combining list/detail/form/sheet/transport and avoid generic components controlled by many unrelated boolean props.

Use `View`, `Text`, `Pressable`, `TextInput`, list primitives, and established design-system components according to their native semantics. Web elements, CSS selectors, hover-only behavior, and browser globals are not portable substitutes.

## Workflow State And Forms

Represent owned initial loading, refreshing, empty, ready, offline/unavailable, validation, forbidden, conflict/stale, submitting, success, disabled, and retry states near the affected region.

Keep editable drafts separate from persisted/server records. Preserve valid values after rejection, associate field/global errors, block duplicate submit, and reconcile returned identity/version/status before navigating or replacing visible data.

Configure keyboard/input behavior deliberately: capitalization, autocorrect, content type, keyboard type, return key, secure entry, multiline behavior, submit sequence, and dismissal. Do not rely on placeholder text as the only label.

## Identity And Interaction

Use stable record IDs for keys, navigation params, selected records, optimistic operations, and action payloads. Bind a press/swipe/menu action to the displayed item rather than mutable global selection or row index.

Respect minimum touch targets, pressed/disabled feedback, gesture conflicts, and screen-reader names/roles/state. An icon-only action needs an accessible label; decorative elements should not create noisy focus stops.

Preserve focus and announcement behavior for validation errors, completed actions, dialogs/sheets, and navigation transitions. Product copy must remain user-facing rather than exposing framework, runtime, delivery, or verification instructions.

## Styling And Layout

Use the repository token/theme/style system. `StyleSheet.create` is useful for stable reusable styles, but inline styles are acceptable for small dynamic values; optimize allocation only on measured hot paths.

Use flex, measured/window dimensions, safe-area insets, and responsive breakpoints appropriate to phones, tablets, split screen, font scaling, and orientation. Avoid hardcoded screen dimensions and assumptions based on one simulator.

Provide image dimensions/aspect behavior, loading/failure fallback, memory-aware sizing, and established caching. Do not ship oversized source assets for small list thumbnails.

## Effects And Native Resources

Dispose listeners, timers, observers, sensors, app-state subscriptions, deep-link handlers, and native module instances. Cancel or order replaceable requests so navigation/filter changes cannot let stale work overwrite current state.

Treat app background/foreground, interrupted permissions, process recreation, and navigation remounts as normal lifecycle events when the task owns those integrations.

## Verification

- Run focused type, lint, component/screen test, Metro, and native build checks supplied by the repository.
- Exercise owned product states, form draft/error/readback, stable action targets, accessibility, font scaling, and representative phone/tablet dimensions.
- Verify native dependency/config changes through the affected Expo prebuild/development-client or native project boundary.
- Check both platforms when the task owns platform differences; otherwise report the actual platform/runtime evidence without claiming parity.
- Confirm listener/resource cleanup by leaving and re-entering the screen or changing the owning dependency.

## Delivery Evidence

Name the screen/component boundary, runtime, stable target, native dependency or lifecycle decision, and visible assertion proving it. Metro startup or one simulator screenshot cannot prove form recovery, platform behavior, accessibility, native integration, or lifecycle safety.

## Unsafe Defaults

- Web DOM/CSS/browser APIs used inside native surfaces.
- Expo and bare-native installation steps mixed without checking the repository runtime.
- Screens hiding infrastructure and navigation behind generic components.
- Row index or mutable selection used as command identity.
- Fixed dimensions copied from one device.
- Every style/callback memoized without a measured boundary.
- Native subscriptions or modules left active after navigation.
