# Next.js Server Actions Quality

This file applies Server Action rules to task-owned mutations, form submissions, revalidation, redirects, and action-driven UI state.

## When To Use

- The task changes Server Actions, action files with `'use server'`, form `action` handlers, `useActionState`, `useFormStatus`, optimistic updates, mutation validation, auth checks, or revalidation after mutation.
- Use this when a route mutation should execute on the server while preserving typed validation, security, and visible form state.
- If the repository uses route handlers or a separate backend API for mutations, preserve that architecture unless the task explicitly moves to Server Actions.

## Implementation Focus

- Keep Server Actions near the feature boundary or in an existing action module. Do not create a catch-all actions file full of unrelated mutations.
- Validate all action inputs before mutation. Treat `FormData` as untrusted and convert it into a typed command or schema-validated object.
- Check authentication, authorization, ownership, and business eligibility inside the server mutation path. Client-side hiding is not a permission boundary.
- Return typed user-actionable errors for validation and expected business failures. Throw only unexpected failures that should reach an error boundary.
- Call `revalidatePath()` or `revalidateTag()` for every cache entry that must change after a successful mutation.
- Use `redirect()` only after successful mutations where navigation is part of the product flow.
- Use `useFormStatus`, `useActionState`, or existing form-state helpers so pending, validation, business error, and success states are visible and scoped to the submitted form.
- Use optimistic updates only when rollback, conflict, duplicate submission, and stale response behavior are clear.
- Do not write uploaded files into ephemeral or public paths without validating type, size, filename, and runtime storage behavior.

## Verification Focus

- Test or probe successful mutation, validation failure, business failure, auth denial, duplicate submission, and revalidation/readback when feasible.
- Verify pending and disabled state belongs to the submitted form, not a neighboring form or stale selected record.
- Run build/typecheck to catch server/client import and action serialization errors.
- For file uploads or cookies, verify runtime constraints and security attributes.

## Evidence Focus

- In the evidence summary, name the action decision: input validation, auth check, error shape, revalidation target, redirect, form pending state, optimistic rollback, file handling, or mutation proof.
