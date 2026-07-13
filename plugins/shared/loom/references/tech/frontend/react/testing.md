# React Testing Quality

This file applies React testing discipline to task-owned components, hooks, state transitions, forms, API-backed views, and provider-wrapped feature surfaces.

Keep this file at the React unit/component boundary. When the task has an MCP-derived browser verification profile, Playwright references own E2E locators, fixtures, network synchronization, viewport execution, and browser artifacts.

## When To Use

- The task creates or changes React components, custom hooks, reducers, forms, API-backed screens, provider setup, or behavior that should be proved through user interaction.
- Use this alongside the repository's existing test runner and testing library. Do not add a new test stack when the repo already has one.
- If no React test infrastructure exists, run the available build/type/lint checks and record the gap instead of inventing unrequested tooling.

## Implementation Focus

- Prefer Testing Library user-facing queries: role, label, placeholder, text, and async `findBy` queries. Test ids are a fallback for controls without accessible semantics.
- Use `userEvent` for realistic interactions: typing, clicking, selecting, tabbing, and submitting forms.
- Wrap components with the same providers they need in production: router, query client, theme, store, i18n, auth, or feature flags.
- Mock network boundaries with the repository's existing approach. MSW-style request mocks are preferred when the repo already supports them.
- Test behavior, not implementation details: rendered state, accessible controls, submitted payloads, visible validation, visible errors, and state transitions.
- For hooks, test the public hook contract through `renderHook` or a small test component. Keep implementation internals private.
- For async behavior, wait for visible outcomes and avoid arbitrary sleeps. Use fake timers only for timer-owned behavior such as debounce.
- Keep tests deterministic by resetting mocks, stores, timers, local storage, and query clients between cases.

## Verification Focus

- Test at least one success path and one meaningful failure or blocking path for forms, mutations, and API-backed views when feasible.
- For provider-backed components, verify provider configuration is part of the test harness rather than duplicated inside the component.
- For hook changes, prove cleanup or cancellation when that is the correctness concern.
- For accessibility-sensitive UI, assert labels, roles, disabled state, and focus-affecting behavior through user-visible queries.

## Evidence Focus

- In the evidence summary, name the React testing decision: accessible query, provider harness, API mock boundary, hook contract, form behavior, async wait strategy, failure path, or cleanup proof.
