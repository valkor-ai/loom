# Next.js Application Implementation

Implement the accepted frontend experience within the repository's Next.js/React version, router mode, rendering/runtime architecture, design system, API contract, and hosting boundary. Do not migrate Pages/App Router or introduce server features from examples unless the technical baseline and task own them.

## Router And Rendering Baseline

Confirm App Router versus Pages Router, React version, server/client runtime, package manager, build scripts, aliases, CSS/component system, and deployment mode before changing files.

For App Router, Server Components are available by default; for Pages Router, use its established page/data APIs. Keep router-specific guidance out of generic component tasks.

Choose rendering and freshness per surface/data lifecycle: static generation, revalidation, dynamic server rendering, streaming, or client-owned refresh. Do not label a page "SSR" merely because it lives in Next.js.

## Server And Client Security Boundary

Server modules may access secrets, database/internal clients, cookies/headers, and private environment values according to accepted architecture. Client modules are public bundles and may receive only serializable, safe values.

Keep `'use client'` at the smallest interactive boundary. Do not mark an entire layout/page client-side to use one hook/button. Use `server-only`/repository guards where appropriate and prevent server dependencies from entering client import graphs.

`NEXT_PUBLIC_*` values are embedded for the browser and are not secrets. Validate required server environment at startup/build/runtime according to when it is consumed.

## Feature Composition

Pages/layouts orchestrate route data, boundaries, metadata, and feature components. Split reusable UI, client islands, server data helpers, mutations, schemas, and formatters by ownership rather than creating generic `utils`/`actions` dumping grounds.

Preserve the accepted API ownership. A separate backend API remains authoritative when selected; do not replace it with route handlers/Server Actions. Full-stack Next features still require clear transaction, auth, validation, and persistence boundaries.

Use typed request/response/view models and stable IDs. Keep database/ORM entities and credentials out of props and responses.

## Product UI State

Render loading, empty, ready, validation, business conflict, forbidden, not-found, unavailable, submitting, success, disabled, and stale states at their owning region/control.

Do not collapse expected failures into thrown generic errors when the user can recover. Error boundaries own unexpected segment failures; form/action state owns expected validation/business outcomes.

Keep drafts separate from persisted data, prevent duplicate writes, and reconcile server-returned identity/version/state. Do not rely only on a toast after mutation.

## Metadata, Images, Fonts, And Assets

Use the router's metadata APIs when public/product metadata is owned. Avoid duplicate data reads between page and `generateMetadata`; share a safe request-scoped/cache helper when accepted.

Use `next/image` for inspectable content images where its optimization model fits; configure actual remote patterns, sizes, dimensions/aspect ratio, priority, loading, and error behavior. Do not hide poor source media behind cropping/blur.

Use `next/font` or the repository font pipeline. Keep static assets under the established public/import boundary and verify base path/CDN behavior.

## Middleware And Route Handlers

Middleware is for lightweight request routing/security/header behavior compatible with its runtime. It must not become a broad business/data layer or accidentally intercept assets, internal Next paths, health, or API routes.

Route handlers implement accepted interfaces only when architecture assigns them. Parse/validate input, authenticate/authorize, invoke an application boundary, and map exact status/body/headers. Do not create duplicate API versions.

## Accessibility, UIX, And Content

Use semantic HTML, labels, focus behavior, keyboard interaction, reduced motion, long/localized content, and the repository's UIX tokens/components. Server rendering does not make inaccessible client interactions acceptable.

Keep product UI free of runtime commands, framework explanations, delivery notes, verification instructions, debug payloads, and implementation progress.

## Verification

- Run focused type/tests and production build when server/client/router/config boundaries change.
- Verify no secret/server-only/database module enters client bundles.
- Exercise task-owned loading/error/not-found/business states and mutation readback.
- Test metadata/image/font/asset behavior only where changed.
- Verify accepted API paths/base binding and middleware exclusions.
- Use browser evidence for real hydration, responsive rendering, navigation, and deployed binding when assigned.

## Delivery Evidence

Identify the Next.js router/rendering/server-client/API decision and the build/route/component/browser assertion proving it. A successful dev render or generated route alone cannot prove production build, secret isolation, hydration, runtime compatibility, or API ownership.

## Unsafe Defaults

- App Router/Server Components imposed on a Pages Router task.
- Whole pages/layouts marked `'use client'` for one interaction.
- Server-only values exposed through public env/props/imports.
- Separate backend contracts reimplemented as route handlers/actions.
- Expected business failures collapsed into generic error boundaries/toasts.
- Middleware matching every path or containing business/database work.
