# React Component Implementation

Implement task-owned React surfaces within the repository's React/framework version, language and type-checking policy, router, design system, data/state libraries, and UIX contract. Do not introduce version/framework patterns because external examples label them modern.

## Component Ownership

Route/page/surface components may orchestrate accepted data, state, navigation, and feature actions. Presentational components receive typed values and emit intent without hidden API/store/router work.

Split components at independent state/behavior/reuse boundaries, not every DOM fragment. Avoid monolithic pages that mix list/detail/form/modal/transport and generic components with many boolean modes.

Model props from runtime/product use. Editable drafts, formatted inputs, pending mutations, and partial filters often need view/form models distinct from backend DTOs.

```tsx
type OrderRowProps = {
  order: OrderRowViewModel
  disabled?: boolean
  onInspect(orderId: string): void
}

export function OrderRow({ order, disabled = false, onInspect }: OrderRowProps) {
  return (
    <button disabled={disabled} onClick={() => onInspect(order.id)}>
      <span>{order.number}</span><span>{order.statusLabel}</span>
    </button>
  )
}
```

Emit stable target identity. Do not derive a submitted record from mutable global selection that can drift after filter/sort/refresh/modal changes.

## Rendering And Identity

Keep render pure: no mutations, subscriptions, network calls, timers, or external writes. Derive values directly or through appropriate memo/selectors rather than mirroring props into state.

Use stable domain keys for insertable/reorderable/filterable/pageable records. Array index is acceptable only for a truly static list with no identity/state.

Do not create component definitions inside another component's render; this remounts state. Inline event closures are not inherently a performance defect; optimize only measured hot boundaries.

## Workflow State And Forms

Represent task-owned loading, empty, ready, validation, conflict/stale, forbidden, unavailable, submitting, success, disabled, and optimistic rollback states near the owning region/control.

Keep editable draft separate from persisted records. Preserve valid input after backend errors, associate field/global errors, block duplicate submit, and reconcile returned identity/version/status.

Use controlled/uncontrolled/form-library patterns consistently. Avoid switching controlled state after mount and avoid storing every derived field twice.

## Errors And Async Boundaries

Error boundaries catch rendering/lifecycle failures below them; they do not catch event-handler/async errors automatically and should not replace expected business state.

Place boundaries at route/feature/independent expensive region granularity with usable recovery. Log unexpected failures once without exposing stack/provider data.

Suspense is appropriate only when the selected framework/data source integrates with it. Loading a promise in an effect is not automatically Suspense behavior.

## Accessibility And UIX

Use semantic elements before ARIA, label fields, name icon-only actions, preserve keyboard/focus behavior, announce meaningful errors/status, and support reduced motion/text/long/localized content.

Use the repository component library and UIX semantic tokens for density, typography, color, spacing, controls, dialogs, tables, and responsive behavior. Do not create custom buttons/dialogs/selects casually.

Product UI must not display runtime commands, stack explanations, delivery progress, verification instructions, or debug messages.

## Security And Browser Boundary

Treat client code/config/storage as public. Never embed secrets, trust hidden controls/routes as authorization, or render unsanitized HTML. Use `dangerouslySetInnerHTML` only with a reviewed trusted/sanitized source and tests.

Preserve accepted API credentials/base paths/CSRF/CORS behavior through the existing client adapter. Do not hardcode local endpoints.

## Verification

- Test visible behavior and emitted intent through roles/labels/text/user events.
- Cover owned workflow states, draft preservation, duplicate blocking, and readback reconciliation.
- Verify target identity after sort/filter/page/refresh/modal open-close.
- Exercise focus, keyboard, semantics, errors, long content, and responsive constraints.
- Run focused type/build/tests when component contracts/imports change.

## Delivery Evidence

Identify component/state/form/accessibility boundary and the visible assertion proving it. A shallow render, private state assertion, or one screenshot cannot prove action identity, async recovery, accessibility, or responsive workflow behavior.

## Unsafe Defaults

- Backend DTOs reused as mutable form/view state indiscriminately.
- API/store/router work hidden in presentational components.
- Render-time side effects or component definitions.
- Dynamic lists keyed by index.
- Every expected failure delegated to an error boundary/toast.
- Custom controls bypassing semantic UIX/design-system primitives.
