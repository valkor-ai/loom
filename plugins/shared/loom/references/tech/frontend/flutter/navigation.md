# Flutter Navigation And Deep Links

Implement task-owned route definitions, parameters, redirects, shell navigation, back-stack semantics, or deep links using the repository's selected router. GoRouter examples are not permission to replace Navigator, AutoRoute, Beamer, or another accepted stack.

## Router Ownership

Construct one application router at a stable composition boundary and inject/watch only the auth/session state needed for redirects. Keep feature route definitions cohesive without competing router instances.

```dart
GoRoute(
  path: '/orders/:orderId',
  name: 'order-detail',
  builder: (context, state) {
    final id = state.pathParameters['orderId'];
    return id == null
        ? const InvalidRoutePage()
        : OrderDetailPage(orderId: id);
  },
)
```

The snippet illustrates parameter validation. Use the selected router's equivalent and preserve accepted paths/names.

## Stack Semantics

Use replace/location navigation for switching canonical destinations and push navigation when users need back-stack return. Do not use `go`/replacement for modal/detail drill-down that must return to filtered list context, or push duplicate shell roots repeatedly.

Define behavior for system back, app-bar back, browser back/forward, tab switching, nested navigators, modals, and unsaved drafts. Keep list filters/scroll/selection restorable when the workflow requires return context.

## Shells And Nested Navigation

Use shell/stateful shell routes for persistent app chrome or independent tab stacks when selected. Give nested navigators stable keys and avoid recreating them on every rebuild.

Shell redirects/loading must not flicker protected content. Keep app bars, navigation rails/bars, and drawer selection derived from route state rather than duplicated booleans/path substring checks.

## Parameters And Data

Pass stable IDs and small serializable route/query values. Validate missing/malformed values before API/storage/state commands and render/redirect according to invalid, not-found, forbidden, and unavailable outcomes.

Do not place large mutable entities, secrets, full form drafts, or authorization decisions in route extras. Extras may not survive process restart, web refresh, or deep links.

Query parameters are appropriate for shareable filter/sort/page/tab context. Encode/decode deliberately and clear stale values when changing workflows.

## Redirects And Auth

Redirect logic must be deterministic and side-effect free: read current session/onboarding/feature state and return a target or null. Do not perform network requests, show dialogs, write storage, or emit analytics inside redirect callbacks.

Handle unknown/loading auth state without redirect loops or flashes. Preserve intended destination safely for login return. Router guards improve UX but do not replace server authorization.

Navigation after provider/bloc commands belongs in a listener/orchestration boundary and should fire once for the accepted success state, not during widget build.

## Deep Links And Platform Setup

Preserve web base path/history fallback and Android/iOS URL schemes, universal/app links, host/path allowlists, and cold/warm-start behavior when deep links are owned.

Reject or safely route unsupported paths and untrusted query values. Deep links must not bypass onboarding/auth/tenant/resource authorization.

Deployment hosting must serve the Flutter web entry point for client routes while preserving API paths; validate direct refresh, not only in-app navigation.

## Verification

- Test initial location, named/path navigation, push/replace, back, nested/shell, and tab-stack behavior owned by the task.
- Exercise valid/missing/malformed params and not-found/forbidden/unavailable states.
- Verify auth loading, redirect, login return, logout, and no redirect loops when changed.
- Test list-detail-return filter/scroll/selection preservation and dirty-draft confirmation.
- Exercise cold/warm deep links and web refresh/back-forward on selected platforms.
- Confirm navigation listeners fire once for the displayed target after state changes.

## Delivery Evidence

Name the route/location and router/widget/integration assertion proving stack, redirect, parameter, or deep-link behavior. A route table or direct callback call cannot prove platform links, browser refresh, nested stacks, or listener deduplication.

## Unsafe Defaults

- GoRouter loaded/introduced from stack availability without navigation ownership.
- Push/replacement semantics chosen interchangeably.
- Large mutable objects or secrets passed through route extras/query.
- Redirect callbacks performing network, storage, dialog, or analytics side effects.
- Router guards treated as authorization.
- Deep links tested only through in-app taps.
