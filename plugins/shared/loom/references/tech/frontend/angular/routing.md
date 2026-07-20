# Angular Router And Navigation

Implement only task-owned navigation: route definitions, route parameters, guards, resolvers, deep links, nested outlets, redirects, or unsaved-change behavior. Component-only work should not receive router guidance.

## Route Ownership

Map each product surface to a stable route segment and keep effective paths aligned with the accepted frontend/API deployment base. Use feature route files and lazy boundaries for cohesive areas rather than one catch-all component that switches on URL text.

```typescript
export const ORDER_ROUTES: Routes = [
  {
    path: '',
    loadComponent: () => import('./order-list.page').then(m => m.OrderListPage),
    title: 'Orders',
  },
  {
    path: ':orderId',
    loadComponent: () => import('./order-detail.page').then(m => m.OrderDetailPage),
    canActivate: [orderAccessGuard],
    resolve: { order: orderResolver },
  },
];
```

Preserve the repository's trailing slash, hash/path location, base href, fallback, and deployment rewrite conventions. Browser refresh/deep link must reach the Angular entry point without breaking `/api` routing.

## Lazy Loading And Preloading

Use `loadComponent`/`loadChildren` for substantial feature boundaries that are not required initially. Avoid tiny chunks for every leaf and avoid eager imports that defeat the lazy boundary.

Preload based on likely user flow and bundle cost. Do not apply `PreloadAllModules` or custom delays from a tutorial without product/runtime rationale. Protect lazy routes server-side as well; code splitting is not authorization.

## Parameters And URL State

Validate path/query values before API/store operations. Missing, malformed, unauthorized, and not-found identifiers need explicit outcomes.

Use `withComponentInputBinding` only when the app has selected it and input names/types align with params/resolved data. Otherwise use `paramMap`/`queryParamMap` with proper lifecycle cleanup or signal interop.

Filters, sort, page, selected tab, and return context belong in query params when they must survive refresh/share/back navigation. Preserve or replace query values deliberately; avoid accidental merge of stale filters into unrelated surfaces.

Do not put secrets, full drafts, or sensitive personal data in URL state. Navigation `extras.state` is ephemeral and should not be the only source for a refreshable route.

## Guards And Authorization

Functional guards may use `inject()` on compatible versions and should return `boolean`, `UrlTree`, or observable/promise equivalents. Return a `UrlTree` for redirects instead of imperative `navigate` plus false.

Guards improve navigation experience; they are not server authorization. Distinguish unauthenticated login redirects, forbidden surfaces, invalid state, and not-found behavior.

Keep unsaved-change guards tied to an explicit dirty-draft contract. Prefer a product confirmation dialog over raw `window.confirm` when the design system provides one, and cover browser/back/close navigation paths.

## Resolvers And Loading Strategy

Use resolvers only for data required before route activation. Long or failure-prone data can render a route-level loading/error state instead. Do not make every page wait on unrelated dashboard requests.

Resolvers must map errors to the accepted route outcome and support cancellation when navigation changes. Returning `null` for every failure erases the distinction among not found, forbidden, and unavailable.

Keep resolver-loaded data, component reloads, and store caches consistent; avoid duplicate requests from all three layers.

## Nested Routes, Outlets, And Titles

Use child routes/outlets when the product hierarchy and preserved context require them. Named outlets add URL and mental complexity; use them for independently navigable panels, not ordinary page layout.

Set route titles/metadata from business context without leaking internal IDs or stale resolved values. Keep breadcrumbs and navigation selection derived from route config/state rather than duplicated path-string checks.

## Navigation Lifecycle

Handle `NavigationCancel` and `NavigationError` as well as start/end when showing global progress. Clean router event subscriptions with `takeUntilDestroyed` or signal interop.

Preserve scroll/focus/restoration behavior for list-detail-return flows. After navigation, place focus at the new page context or restored control according to accessibility/product behavior.

## Verification

- Test exact route matching, redirects, lazy imports, params/query parsing, and wildcard/not-found behavior.
- Exercise guard allow/redirect/forbid and resolver success/not-found/forbidden/unavailable branches.
- Verify direct deep-link refresh and deployment fallback for changed public routes.
- Confirm filter/tab/page/return context through forward, back, refresh, and programmatic navigation.
- Test dirty-draft navigation and focus/scroll restoration when owned.
- Build route configuration to catch circular/missing standalone imports.

## Delivery Evidence

Identify the effective URL, route owner, and RouterTestingHarness or browser assertion proving activation and relevant guard/resolver/query behavior. A route object or direct guard call alone cannot prove lazy loading, redirects, navigation cancellation, deep links, or deployment fallback.

## Unsafe Defaults

- Router reference selected from prose rather than navigation ownership.
- Guards treated as server authorization.
- Imperative navigation inside guards instead of returning `UrlTree`.
- Resolver failures collapsed to null/home redirects.
- Filters/drafts duplicated in hidden component state when URL persistence is required.
- Named outlets and preloading added without a workflow reason.
- Deep links tested only through in-app clicks.
