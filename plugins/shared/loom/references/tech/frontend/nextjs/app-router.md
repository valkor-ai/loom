# Next.js App Router

Apply this reference only when TechnicalBaseline selects App Router and the task owns route definitions, segments, layouts, navigation, boundaries, route handlers, or metadata. Generic Next.js and component-only tasks must not receive it.

## Segment Ownership

Map product surfaces to clear segments. Use route groups for code/layout organization without URL changes, not to hide unrelated workflows.

```text
app/
  (workspace)/
    layout.tsx
    orders/
      page.tsx
      loading.tsx
      error.tsx
      [orderId]/
        page.tsx
        not-found.tsx
```

The root layout owns required document shell/providers. Nested layouts persist across child navigation; `template.tsx` remounts and should be used only when reset-on-navigation is intended.

Keep route-specific UI/data near the segment and shared product components outside route folders according to repository convention. Avoid circular imports between layout and feature modules.

## Dynamic, Catch-All, And Search Params

Type and validate params/search params before data/actions. Account for the selected Next version's async params API. Missing/malformed/unauthorized/not-found states require explicit outcomes.

Use catch-all/optional catch-all only for real hierarchical content. Do not create broad dynamic routes that swallow static paths or internal assets.

Query/search params are suitable for shareable filters, sort, page, tab, and return context. Parse/allowlist them server-side and preserve/clear intentionally during navigation.

## Loading, Error, And Not Found

Place `loading.tsx` at an async segment boundary where fallback helps; avoid replacing the full working shell for a small slow region. Use Suspense for independently streamable regions.

`error.tsx` is a Client Component and owns unexpected segment failures plus reset/retry. It should log through the selected boundary without exposing stack/provider data.

Call `notFound()`/provide `not-found.tsx` for genuine missing records. Do not convert forbidden/unavailable/validation outcomes into not found unless the disclosure contract says so.

Expected business errors should render task-owned states rather than throw into a generic error boundary.

## Parallel And Intercepting Routes

Use parallel routes for independently navigable/rendered slots with explicit defaults and refresh behavior. Use intercepting routes for product-approved modal/detail navigation that also has a direct full-page URL.

These patterns add back-stack, default, refresh, and accessibility complexity. Provide close/back/focus behavior and direct deep-link fallback; do not use them for ordinary layout columns.

## Navigation And Redirects

Use links for navigable destinations and router methods for event-driven transitions. Preserve list/filter/scroll context for detail-return workflows.

Use server `redirect` for server-known outcomes and client router navigation only at interactive boundaries. Avoid mount effects that redirect after flashing protected/wrong content.

Define middleware/auth redirects without loops and retain a safe intended destination. Navigation checks do not replace server authorization.

## Route Handlers

Route handlers are server HTTP interfaces. Implement only the accepted method, path, schemas, statuses, errors, authorization, and exposure behavior. Validate inputs, scope identity/tenant, map failures, and keep persistence/business logic in the accepted application boundary.

Avoid creating route handlers solely to proxy an existing same-origin backend unless architecture requires a BFF. Preserve cookies, streaming, caching, headers, and body limits deliberately.

## Metadata

Use static metadata for stable pages and `generateMetadata` for dynamic accepted content. Deduplicate data reads safely and provide canonical/OpenGraph/robots only where product/SEO owns them.

Never expose private record details, internal IDs, or failed lookup messages in metadata. Metadata failures need bounded behavior.

## Verification

- Run production build for changed route file contracts and server/client boundaries.
- Exercise exact static/dynamic/catch-all/query paths, redirects, and not-found/forbidden outcomes.
- Verify loading streaming, error reset, and expected business state placement.
- Test layout persistence versus template remount and list-detail-return context.
- Exercise parallel/intercepting direct refresh, back/close, and focus when owned.
- For route handlers, assert exact HTTP contract and auth/failure branches.

## Delivery Evidence

Name the segment/effective URL and build/route/browser assertion proving activation, persistence, boundary, redirect, metadata, or handler behavior. File presence alone cannot prove matching precedence, deep-link refresh, streaming, back stack, or deployment fallback.

## Unsafe Defaults

- App Router reference selected without an accepted App Router signal and navigation task.
- Route groups/dynamic catch-alls used to hide unclear ownership.
- Full-shell loading fallback for one slow region.
- Every failure converted to `notFound()` or generic `error.tsx`.
- Parallel/intercepting routes used for ordinary page layout.
- Route handlers duplicating an accepted backend contract.
