# UIX Stack: Vue

Use for Vue, Nuxt, Vite Vue, and related component-driven Vue projects.

## Structure

- Follow existing Nuxt/Vue routing, layout, composable, and component conventions.
- Separate app shell/layout, page views, business components, composables, and reusable UI primitives.
- Use the existing design system or token approach before adding a new one.

## Implementation Rules

- Keep reactive state scoped: page/query state, form state, selected record, and modal/drawer state should not conflict.
- Use computed values for derived UI labels, eligibility, and filtered data.
- Keep async loading/error states near the view that depends on them.
- Use slots/components for repeated table actions, status badges, field rows, and empty/error states.
- Preserve accessibility attributes on custom controls.

## Nuxt Notes

- Use layouts for persistent shells.
- Keep server/client-only code separated.
- Avoid putting delivery commands, build notes, or stack explanations into pages.

## Verification

- Run focused build/type/lint commands when present.
- Render and check responsive behavior.
- Verify forms and transitions preserve user input and focus.
