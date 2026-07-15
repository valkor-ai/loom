# React Native Navigation

Apply navigation guidance only when the task owns routes, stacks, tabs, drawers, modals/sheets, route parameters, deep links, protected flow, state restoration, or back behavior. Preserve the repository's Expo Router or React Navigation model.

## Navigation Ownership

Map product destinations and transitions before editing navigator code. Distinguish root/auth/onboarding/tab/detail/modal flows, define which navigator owns each route, and preserve the user's expected return location.

Do not introduce Expo Router into a React Navigation app, or a parallel manual navigator into an Expo Router app, for one feature. Use the installed version's APIs and typed-route support.

Keep reusable screens independent of file-based route mechanics where practical. Route wrappers parse params, provide route-owned dependencies, and render the feature surface.

## Route Parameters

Pass stable IDs and small serializable navigation state. Avoid full mutable records, functions, secrets, access tokens, or large payloads in params.

Treat external/deep-link params as untrusted. Normalize singular/array forms where the router permits both, validate format/range, and render not-found/invalid/forbidden states before data or native operations.

Type route names and params through the repository's navigation types or generated Expo typed routes. Do not silence route typing with broad casts.

## Push, Replace, Reset, And Dismiss

Use push when history should preserve the current screen; replace for one-way transitions such as completed auth/onboarding; reset only when old history must become unreachable; dismiss modal stacks intentionally.

Prevent duplicate destinations from repeated taps or async completion. Keep pending state on the initiating control and reconcile navigation only after the required command result.

Preserve list filter/scroll/context when users inspect a detail and return. Do not reconstruct return state from mutable global selection if route identity already exists.

## Layouts, Tabs, And Modals

In Expo Router, route groups organize navigation without changing URL segments; they are not authorization by themselves. `_layout` owns shared providers, guards, and navigator options at the smallest coherent scope.

Keep tab identity stable and avoid nesting navigators merely to hide headers. Modal/sheet presentation needs explicit close/back behavior, focus/announcement handling, unsaved-change policy, and safe-area/keyboard integration.

Use product labels and accessible names for headers, tabs, and actions. Icons alone must have understandable labels and selected state.

## Protected Flows

Gate at an owning layout/navigator while authentication state is known. Render a stable loading/restoration surface before redirecting so the app does not flash protected content or bounce between routes.

Authorization remains server-enforced. A hidden tab or redirect is presentation, not access control. Preserve the intended destination across sign-in only when it is safe and still authorized.

Handle logout/account/tenant switch by clearing incompatible navigation and persisted state so back navigation cannot reveal stale screens.

## Deep Links And External Entry

Keep schemes, universal/app links, linking config, route patterns, and platform association files aligned. Define cold start, warm app, authenticated, unauthenticated, invalid target, and unavailable record behavior.

External links must not bypass validation or route guards. Avoid open redirects and restrict callback destinations to accepted origins/routes.

## Back And Unsaved Work

Coordinate header back, gestures, Android hardware back, modal close, and system navigation. Intercept only when the task owns unsaved work or an overlay; return control to the navigator otherwise.

Do not register competing back handlers at several layers. Clean up listeners and ensure the topmost visible surface owns interception.

## Verification

- Test direct entry, forward/back, tab switching, modal open/dismiss, repeated navigation, and return-context preservation.
- Exercise valid, missing, malformed, array-shaped, unauthorized, and not-found params where applicable.
- Verify auth restoration, protected redirect, post-login destination, logout reset, and account/tenant switch.
- Test cold/warm deep-link entry and config changes on affected platforms.
- Check Android hardware back and iOS gesture/header behavior when custom interception exists.

## Delivery Evidence

Name the navigator/layout owner, route/param contract, history operation, guard/deep-link path, and assertion proving return/back behavior. A route file existing or one successful `push` does not prove protected entry, restoration, malformed params, or platform back semantics.

## Unsafe Defaults

- A second navigation model introduced for one feature.
- Mutable records or credentials passed through params.
- Broad casts used to bypass typed route errors.
- Leaf-screen redirect logic duplicated across protected routes.
- `replace`/reset used where users must return to prior context.
- Route groups treated as authorization.
- Back listeners retained after the owning screen disappears.
