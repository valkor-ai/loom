# Next.js Server Mutations And Actions

Apply Server Action guidance only to an App Router task that explicitly owns a server-side form/action mutation. Preserve a selected backend/route-handler architecture unless the accepted design assigns the mutation to a Server Action.

## Action Boundary

Place actions with their feature or focused server action module. Every action is a remotely invokable server entry point: authenticate, authorize, validate, scope tenant/ownership, and enforce business eligibility inside the action/application path.

Treat `FormData` and programmatic arguments as untrusted. Parse with a typed schema/command and preserve field/global error shape.

```tsx
'use server'

export async function approveOrder(
  previous: ApprovalState,
  formData: FormData,
): Promise<ApprovalState> {
  const actor = await requireActor()
  const input = ApprovalSchema.safeParse(Object.fromEntries(formData))
  if (!input.success) return validationState(input.error)

  const result = await orders.approve(actor, input.data)
  if (!result.ok) return businessFailureState(result.error)

  revalidateTag(orderTag(result.value.id))
  return { kind: 'succeeded', order: result.value }
}
```

Use module-level `'use server'` or inline actions according to repository conventions. Do not create catch-all action files or expose generic database operations.

## Form And Client State

Use `useActionState`, `useFormStatus`, or the selected form library to render pending, field validation, business conflict, forbidden, unavailable, and success state at the submitted form/control.

`useFormStatus` must be rendered beneath the owning form. Prevent duplicate submits and preserve draft values after expected failures. Multiple row/forms need stable target identity; never rely on a mutable selected record.

Progressive enhancement should work where accepted, and redirect/navigation should occur only after successful durable mutation.

## Authorization, CSRF, And Origin

Session/cookie mutations require the framework/deployment's origin and CSRF protections plus explicit authorization. Do not assume hidden action IDs or same component placement secure an action.

Validate resource ownership/tenant/state after loading current data and enforce database constraints for concurrent integrity. Never trust actor/tenant/initial state from form fields.

Rate limiting/idempotency/audit belongs to the accepted security/API/application design, especially for sensitive or repeatable programmatic calls.

## Transaction And Side Effects

One application operation owns transaction and external-effect ordering. Do not perform unrelated database writes directly across action modules or hold transactions open during file/email/provider calls.

For file upload, enforce size/count/type/content/name, stream to accepted durable storage, scan where required, and never write to ephemeral/public app paths as permanent storage.

Cookies must use accepted secure/httpOnly/sameSite/path/domain/expiry behavior and should not contain sensitive payloads.

## Revalidation And Readback

After success, invalidate every affected path/tag/cache/read model, but no broader. Stable domain-owned tags are preferable to ad hoc strings.

Revalidation is not UI state reconciliation by itself. Ensure current form/list/detail receives returned or refetched identity/version/status/count and does not stay stale.

Use `redirect` after mutation only when navigation is the accepted outcome. Remember redirect throws control flow; do not catch it in a broad action catch.

## Optimistic Updates

Use optimistic UI only for predictable operations with stable temporary/target identity, duplicate prevention, rollback/conflict handling, stale-response ordering, and accessible pending/failure feedback.

Do not optimistically confirm destructive, high-conflict, authorization-sensitive, or irreversible work without an accepted design.

## Failure Mapping

Return serializable typed expected failures. Throw unexpected failures to the route error boundary/logging path. Never return raw database/provider/token/stack messages.

Broad catch blocks must not swallow redirects/notFound or convert programming failures into user validation messages.

## Verification

- Test successful mutation plus validation, auth, ownership, conflict, duplicate, unavailable, and unexpected paths owned by the action.
- Verify exact target identity and server-owned fields cannot be spoofed.
- Prove transaction/readback and affected tag/path revalidation without unrelated invalidation.
- Exercise pending/disabled/draft/error/success state for multiple forms/rows.
- Verify redirect/cookie/file behavior and limits where changed.
- Run production build for action serialization/server-client boundaries.

## Delivery Evidence

Name the action/application operation and the mutation, auth, state, and revalidation/readback assertions proving it. A form invoking an action or `revalidatePath` call alone cannot prove validation, authorization, durable write, cache coherence, or duplicate handling.

## Unsafe Defaults

- Server Action introduced when a separate backend/handler owns mutations.
- Raw FormData/object spread passed into persistence.
- Actor/tenant/resource state trusted from hidden fields.
- Generic catch swallowing redirect/notFound/programming errors.
- Revalidation treated as sufficient visible readback.
- File writes to ephemeral/public paths and optimistic destructive operations without rollback.
