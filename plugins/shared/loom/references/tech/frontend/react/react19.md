# React 19 Quality

This file applies React 19 feature discipline to task-owned forms, `use()`, action state, optimistic updates, and ref-as-prop changes.

## When To Use

- The task explicitly uses React 19 features such as `use()`, `useActionState`, `useFormStatus`, `useOptimistic`, ref as prop, or action-based form patterns.
- Use this when the repository's React version and framework support the feature. Do not add React 19-only APIs to older projects.
- If the task can be solved with established repository patterns and React 19 is not already available, do not introduce these features.

## Implementation Focus

- Confirm package versions, framework support, TypeScript types, and lint rules before using React 19 APIs.
- Use `use()` only where Suspense semantics are intended and the framework/runtime supports reading the value in render.
- Keep `useActionState` form state small, typed, and user-actionable. Return validation and business-blocking errors in a shape the form can render near the affected controls.
- Use `useFormStatus` inside the submitted form subtree so pending state reflects the actual form action.
- Use `useOptimistic` only when the temporary state is clearly reversible or reconcilable after server response.
- For optimistic updates, handle conflict, rejection, duplicate submission, and stale server response paths explicitly.
- Use ref-as-prop only when the repository has adopted React 19 typing; otherwise preserve the existing `forwardRef` convention.
- Keep progressive enhancement in mind for server action forms: required fields, disabled states, pending labels, error messages, and final readback should still be visible.

## Verification Focus

- Run the framework build/typecheck that proves React 19 APIs are available.
- Test pending, success, validation failure, business failure, and optimistic rollback paths when touched.
- Verify server action or mutation behavior updates the visible view and does not leave stale optimistic state.
- Check that form accessibility, disabled states, and error messaging still work while pending.

## Evidence Focus

- In the evidence summary, name the React 19 decision: `use()` Suspense read, action state shape, form pending behavior, optimistic reconciliation, ref convention, or version compatibility proof.

