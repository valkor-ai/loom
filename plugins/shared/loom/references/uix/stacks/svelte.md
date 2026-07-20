# UIX Stack: Svelte

Use for Svelte, SvelteKit, and related component-driven projects.

## Structure

- Follow existing route, layout, store, and component conventions.
- Keep page orchestration, business components, reusable UI primitives, and data modules separated.
- Use SvelteKit load/actions or the repo's data approach consistently.

## Suggested Split

```text
src/routes/
src/lib/components/ui/
src/lib/components/feature-name/
src/lib/server|services/
src/lib/styles/tokens.css
```

## Implementation Rules

- Represent loading, empty, error, validation, success, and business-blocking states directly in the template.
- Use stores only when state is shared across surfaces; keep local state local.
- Use semantic tokens in CSS variables, app CSS, Tailwind, or the existing styling system.
- Keep transitions purposeful and respect reduced motion.
- Avoid hiding product behavior inside overly clever reactive statements.
- Merge token templates into existing app CSS, Tailwind config, or SvelteKit layout assets. Do not create per-component parallel token blocks.
- Keep business actions close to the component that displays the affected object, so success/error can update in place.

## Template Pattern

```svelte
{#if state.status === 'loading'}
  <SkeletonRows />
{:else if state.status === 'error'}
  <ErrorState message={state.message} />
{:else if state.status === 'empty'}
  <EmptyState />
{:else}
  <DataTable rows={state.data} />
{/if}
```

## Verification

- Run focused build/type/lint commands when present.
- Render workflows and check state transitions, focus, and responsive behavior.
- Confirm stores, forms, and transitions preserve task context across loading/error/success states.

## Page Load And Action Boundary

For SvelteKit, page data, form actions, layouts, and server/client boundaries
belong to the framework engineering contract. UIX owns how their states appear
and how the user keeps context while moving through the surface.

```text
layout shell -> page data state -> feature region -> form/action
-> pending/validation/result -> invalidate or reconcile affected region
```

- Keep a persistent shell in the layout and keep route-owned content in the page or feature component.
- Render `loading`, `empty`, `error`, `validation`, `success`, and `business-blocking` states beside the region or action they explain.
- Use stores for genuinely shared UI state such as navigation or cross-route filters; keep selected records and drafts local to the owning surface when possible.
- After an action, invalidate or update the exact affected data and preserve filters, selection, and return context.
- Keep transitions purposeful and bounded. A transition must not hide a state change, move the primary action, or block keyboard focus.
