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
