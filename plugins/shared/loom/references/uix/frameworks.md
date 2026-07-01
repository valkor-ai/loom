# Frontend Framework UIX

Load this file when a framework, component library, design system, or frontend stack is named.

## Shared Rules

- Follow the existing routing, component, styling, data-fetching, form, and test conventions before adding new patterns.
- Use the existing design tokens and component primitives where they exist. If none exist, keep the new system small and coherent.
- Keep visual behavior close to framework idioms: server/client boundaries, hydration, Suspense/loading paths, route transitions, and error boundaries should match the stack.

## Focused Stack References

Load a focused stack reference when the MCP `referenceIds` asks for it or the task clearly uses that stack.

- `uix.stacks.react` -> `references/uix/stacks/react.md`
- `uix.stacks.vue` -> `references/uix/stacks/vue.md`
- `uix.stacks.plain_html` -> `references/uix/stacks/plain-html.md`
- `uix.stacks.native_mobile` -> `references/uix/stacks/native-mobile.md`
- `uix.stacks.threejs` -> `references/uix/stacks/threejs.md`

## Web Frameworks

- React/Next.js/Remix: preserve component boundaries, accessible semantics, loading/error routes, and client/server data ownership.
- Vue/Nuxt: follow composition patterns, route-level loading/error behavior, and existing store conventions.
- Svelte/SvelteKit: keep state local where possible, use route data and progressive enhancement when present.
- Angular: follow module/standalone component conventions, reactive forms, and service boundaries.

## UI Libraries

- Tailwind: use project tokens and utility composition consistently; avoid one-off arbitrary values when a token exists.
- shadcn/Radix/Headless UI: preserve accessible primitives and do not remove keyboard/focus behavior while restyling.
- MUI/Ant/Chakra: use theme overrides and library variants before hand-rolling replacements.

## Native And Cross-Platform

- React Native/Expo and Flutter work should honor platform navigation, safe areas, input behavior, gestures, and offline/loading states.
- Record any framework-specific manual checks that cannot be automated in the TaskResult.
