# Angular Application Implementation

Implement the accepted frontend experience within the repository's Angular version, application bootstrap, design system, API contract, and feature boundaries. Angular 17+ patterns are available only when the selected project version supports them.

## Application Composition

Preserve standalone versus NgModule architecture. New standalone features should use `ApplicationConfig`, `bootstrapApplication`, route/provider functions, and explicit component imports; an established NgModule application should not be partially rewritten unless migration is task-owned.

Register application-wide providers once in the composition root. Keep feature services/state/routes close to their owning capability and avoid `providedIn: 'root'` for stateful providers that should have route/feature lifetime.

Use injection tokens for configurable ports and browser/runtime abstractions. `inject()` and constructor injection are both valid; follow repository style and use `runInInjectionContext` only when a function genuinely needs an injection context.

## State Boundary

Choose state by lifetime and sharing needs:

| State | Suitable owner |
|---|---|
| Local visual/edit state | component signals/form model |
| Derived local state | `computed()` |
| Reusable feature operation | service/facade |
| Shared cross-surface lifecycle | selected store such as NgRx |
| URL-shareable filters/selection | router params/query params |
| Server source of truth | API service/cache policy, not duplicated client truth |

Signals are synchronous state primitives. Use `signal`, `computed`, and controlled `effect` for local/derived state; avoid effects that copy one signal into another, issue uncontrolled writes, or hide dependency cycles.

```typescript
readonly records = signal<readonly RecordSummary[]>([]);
readonly selectedId = signal<string | null>(null);
readonly selected = computed(() =>
  this.records().find(record => record.id === this.selectedId()) ?? null,
);
```

Keep persisted records immutable enough for OnPush/signal equality to be meaningful. Maintain a separate editable draft and reconcile it after accepted save/readback.

## API And Error Boundary

Centralize HTTP transport in typed services/adapters. Preserve accepted method, path, payload, status, error, auth, pagination, and same-origin/base URL rules. Do not duplicate endpoint strings across components.

Use interceptors for cross-cutting transport concerns such as credentials, correlation, or error normalization only when they apply broadly. Business-specific error mapping belongs in the feature service/facade.

Distinguish validation, conflict/stale state, permission denial, not found, unavailable dependency, and transport failure in UI state. Do not convert every failure to an empty list or generic toast.

## Rendering And Change Detection

Use `ChangeDetectionStrategy.OnPush` for task-owned business components where compatible. Update signals/immutable inputs through explicit events and avoid manual `detectChanges`/`markForCheck` as a routine state mechanism.

Use `@if`, `@for`, `@switch`, and deferred views only on compatible Angular versions. Track dynamic collections by stable domain identity, never mutable array index. `@defer` needs loading, placeholder, error, and triggering behavior that does not hide primary work.

Keep expensive transformation out of templates. Use computed view models, pure pipes, selectors, or bounded service projections. Do not call APIs or mutate state from template getters.

## Forms And Workflow State

Use reactive forms for business workflows with validation, nested structures, dynamic rows, and explicit submit lifecycle. Typed forms should model nullability and disabled controls correctly; `form.value` may omit disabled fields while `getRawValue()` includes them.

Client validation improves feedback but does not replace server rules. Map backend field/global errors without discarding the user's draft, and clear stale errors when relevant fields change or a resubmit succeeds.

Represent loading, empty, ready, submitting, success, disabled, and business-blocking states at the owning region/control. Prevent duplicate writes while preserving retry/recovery.

## Security And Content

Treat all browser code/config as public. Never embed secrets or rely on route guards/UI visibility as server authorization. Sanitize or avoid untrusted HTML; use Angular's binding model and do not bypass security with `DomSanitizer` without a reviewed source contract.

Keep product surfaces free of runtime commands, framework explanations, delivery progress, verification instructions, and implementation notes. Use the accepted UIX tokens/components and business language.

## Verification

- Build/typecheck the affected Angular project and compile templates/imports/providers.
- Exercise task-owned loading, empty, ready, validation, conflict, permission, unavailable, submitting, and success states.
- Verify immutable updates, stable tracking, draft preservation, and readback reconciliation.
- Test typed HTTP mapping and exact error/status behavior when API binding changes.
- Confirm no sensitive configuration or unsafe HTML path was introduced.

## Delivery Evidence

Identify the Angular composition, state, HTTP, form, or rendering decision and the public behavior/assertion proving it. Compilation or a screenshot alone does not prove API mapping, failure recovery, state ownership, or form semantics.

## Unsafe Defaults

- Standalone/NgModule migration mixed into unrelated feature work.
- Root-scoped mutable feature state by convenience.
- Effects used to mirror derived signal state.
- API calls and business error handling inside components.
- Array index used to track mutable business rows.
- Manual change detection used to compensate for unclear ownership.
- Browser environment files treated as secret storage.
