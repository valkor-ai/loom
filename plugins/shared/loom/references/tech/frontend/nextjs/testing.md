# Next.js Testing

Use Next.js framework testing only for tasks that own tests. Choose the smallest proof for route/server/client/action/runtime behavior. MCP-assigned Playwright references own real browser navigation, hydration, multi-viewport rendering, and deployed workflow evidence.

## Proof Boundary

| Claim | Suitable proof |
|---|---|
| Pure parser/mapper/schema | TypeScript unit test |
| Client component behavior | selected React component test tooling |
| Server data/helper | server unit/integration test with owned ports |
| Server Action | action/application integration pattern |
| Route handler | Request/Response HTTP contract test |
| Route file/build boundary | production `next build` plus focused route test |
| Hydration/navigation/rendered workflow | assigned browser test |

Do not render every Server Component with brittle framework internals or claim component tests prove middleware, deployment rewrites, browser hydration, streaming, or server/client bundles.

## Client Components

Test visible behavior with accessible queries and realistic events through the repository's React runner. Provide router/action/data adapters at owned boundaries without mocking the component/state logic being claimed.

Cover pending, validation, conflict, forbidden, unavailable, disabled, success, optimistic rollback, and target identity where relevant. Avoid private state/hook implementation assertions.

## Server Components And Data

Test pure server helpers and application/data boundaries directly. For server components, prove authorization/scoping, expected outcome mapping, and serialization where the repository has a stable harness; production build remains required for import/serialization contracts.

Use deterministic request identity, cache state, time, and data. Reset/invalidate shared/request caches between tests so order does not affect outcomes.

Mock/fake external services, clocks, and selected repositories at ports. Do not mock the authorization, cache key, or mapping behavior being tested.

## Server Actions

Exercise typed input parsing, authentication, ownership, business validation/conflict, successful durable mutation, duplicate handling, returned serializable state, and exact revalidation/readback.

Do not unit-test only the exported function while bypassing framework form/cookie/redirect behavior if that behavior is the claim. Preserve thrown redirect/notFound semantics in the harness.

## Route Handlers And Middleware

Construct real `Request` objects and assert status, headers, content type, body/error shape, cookies, auth, and method/path behavior. Test body/size/format limits when owned.

Middleware matcher/redirect/rewrite/header behavior requires focused integration/build/runtime or browser probes; a direct function call does not prove matcher exclusions or effective path topology.

## App Router Boundaries

Exercise dynamic/search params, loading/error/not-found, redirects, and metadata through stable repository tooling where available. Production build catches route file contracts, server/client imports, action serialization, and runtime incompatibility.

Parallel/intercepting/deep-link/back-stack behavior belongs in browser tests when it requires actual navigation/history/focus.

## Runtime And Environment

For config/runtime tasks, build/start with valid and invalid required env, then probe exact headers/rewrites/health/image/runtime behavior. Ensure tests do not print secrets.

Inspect client output/import graph when public/server env or server-only dependency isolation is the claim. Use bundle evidence only for measured performance tasks.

## Verification And Cleanup

Run the changed test target first, then production build when routes, server/client boundaries, actions, middleware, metadata, config, or runtime changed. Do not invent a test runner/coverage threshold for a small task absent repository policy.

Reset mocks, caches, environment, cookies, timers, test servers, database fixtures, and global fetch implementations. Avoid arbitrary sleeps and test-order dependencies.

## Delivery Evidence

Record boundary, scenario, command, and meaningful HTTP/visible/build assertion. Passing counts or `next build` alone cannot prove business/auth branches, cache coherence, browser hydration, navigation history, responsive UI, or deployed binding.

## Unsafe Defaults

- Load this reference only when the accepted task owns Next.js test creation, test modification, or test-specific verification.
- Client component tests claimed as Server Component/runtime/middleware proof.
- Action tests bypassing auth/transaction/revalidation/readback.
- Shared cache/env/cookie state leaking across tests.
- Build success used as the only behavioral evidence.
- Browser behavior asserted without an actual browser boundary.
