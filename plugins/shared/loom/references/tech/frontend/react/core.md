# React Core Quality

This file applies React implementation discipline to task-owned components, screens, feature modules, and React application surfaces.

## When To Use

- The task creates or changes React components, route/page components, feature screens, forms, lists, drawers, modals, or UI orchestration.
- Use this for React-specific component boundaries, rendering behavior, accessibility, error handling, and repository adaptation.
- If the task only changes CSS tokens or visual layout rules, use UIX references as the authority and keep this file focused on React implementation mechanics.

## Implementation Focus

- Follow the repository's existing router, folder layout, component library, styling system, test runner, and data fetching conventions before introducing new patterns.
- Keep route/page components as orchestration once a surface has real workflow complexity. Extract feature components, UI primitives, API/client helpers, formatters, validators, and state helpers when they are reused or make the page hard to inspect.
- Use TypeScript component props that match runtime behavior. Do not reuse server DTOs as editable form state when the UI can hold partial, invalid, or formatted values.
- Keep business state transitions and client-visible domain rules explicit. Do not collapse business-blocking outcomes into generic error banners when the user needs action-specific feedback.
- Use stable, domain-owned keys for lists. Do not use array index keys for dynamic rows, filtered lists, sorted lists, or tables with mutations.
- Keep event handlers, derived labels, and eligibility checks close to the owning feature, but move repeated logic into typed helpers or hooks when it appears in multiple surfaces.
- Add error boundaries or route-level error handling for production surfaces that perform async work or render user-owned data. Small leaf components may rely on an existing parent boundary.
- Use semantic HTML and accessible names for controls. Icon-only buttons need accessible labels or existing design-system equivalents.
- Keep product UI free of delivery notes, runtime commands, framework explanations, and verification instructions.

## Verification Focus

- Run the repository's focused React build, typecheck, lint, and component test commands when available.
- Test meaningful user behavior with accessible queries: loading, empty, ready, validation error, business-blocking error, submitting, success, and disabled states touched by the task.
- For form or mutation work, prove that the displayed record and submitted record cannot drift apart through stale closures or stale selected state.
- For list/table work, verify stable identity, sorting/filtering behavior, action targeting, empty state, and error state.
- For async work, verify cancellation or stale-result handling when navigation, filter changes, or repeated submissions can supersede an earlier request.

## Evidence Focus

- In the evidence summary, name the React boundary decision: route/page orchestration, extracted feature component, typed view model, action-specific business state, error boundary, stable list identity, or accessible control behavior.

