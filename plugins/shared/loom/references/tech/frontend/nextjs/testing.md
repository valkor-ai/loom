# Next.js Testing Quality

This file applies Next.js testing and verification rules to task-owned routes, Server Components, Client Components, Server Actions, route handlers, and runtime configuration.

Keep this file at the Next.js route/component/server boundary. When the task has an MCP-derived browser verification profile, Playwright references own E2E locators, fixtures, network synchronization, viewport execution, and browser artifacts.

## When To Use

- The task changes App Router pages, layouts, route handlers, Server Actions, metadata, loading/error/not-found states, middleware, or Next.js configuration.
- Use this alongside the repository's existing test runner and React testing strategy. Do not introduce a new test stack for a small task.
- If the project has no route/component tests, run build/type/lint checks and record the test gap rather than inventing unrelated infrastructure.

## Implementation Focus

- Prefer tests around user-visible route behavior, not file names alone: ready state, loading fallback, error reset, not found, redirect, form pending, validation error, and successful mutation.
- For Client Components, use accessible queries and realistic user events through the existing React test tooling.
- For Server Actions, test validation, auth/ownership denial, successful mutation, revalidation target, and returned error shape through an existing action or integration-test pattern.
- For route handlers, test status codes, request parsing, response DTO shape, and expected error mapping.
- Keep server-only dependencies mocked at owned boundaries. Do not mock the component or action under test.
- Use `next build` as the required integration proof for server/client boundaries, metadata, config, middleware, and App Router file contracts.
- Keep test data deterministic and isolated. Do not depend on route execution order or shared mutable caches between tests.

## Verification Focus

- Run the focused test command, then `next build` when framework boundaries changed.
- Verify no secret, database client, or server-only helper appears in client-bundle paths.
- For mutation flows, verify visible readback or cache invalidation when feasible.
- For config changes, verify the exact affected route, header, image, env, or runtime behavior.

## Evidence Focus

- In the evidence summary, name the proof type: route behavior test, Server Action test, route handler test, Client Component interaction, metadata proof, build proof, or runtime config proof.
