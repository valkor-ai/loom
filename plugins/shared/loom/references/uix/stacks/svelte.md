# UIX Stack: Svelte

Use for Svelte, SvelteKit, and related component-driven projects.

## Structure

- Follow existing route, layout, store, and component conventions.
- Keep page orchestration, business components, reusable UI primitives, and data modules separated.
- Use SvelteKit load/actions or the repo's data approach consistently.

## Implementation Rules

- Represent loading, empty, error, validation, success, and business-blocking states directly in the template.
- Use stores only when state is shared across surfaces; keep local state local.
- Use semantic tokens in CSS variables, app CSS, Tailwind, or the existing styling system.
- Keep transitions purposeful and respect reduced motion.
- Avoid hiding product behavior inside overly clever reactive statements.

## Verification

- Run focused build/type/lint commands when present.
- Render workflows and check state transitions, focus, and responsive behavior.
