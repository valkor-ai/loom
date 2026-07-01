# UIX Focus: Frameworks

Load this when choosing how to apply a UI contract in a specific frontend stack. For MCP-driven work, prefer stack references from `uiQualityContract.referenceProfile.referenceIds`.

## Rule

Frameworks implement the UI contract; they do not redefine the product surface. Keep scenario, quality level, states, and forbidden user-visible content aligned with the MCP artifact.

## Component Architecture

- Use existing routing, data fetching, component, styling, and test conventions.
- Separate app shell, feature screens, shared primitives, API/data modules, and utilities.
- Avoid one giant component once the workflow includes table/list, form, modal/drawer, and async states.
- Keep design tokens in the framework's normal location: CSS variables, Tailwind config, theme provider, app CSS, or native theme.

## Framework-Specific References

- React/Next/Vite React: `references/uix/stacks/react.md`.
- Vue/Nuxt: `references/uix/stacks/vue.md`.
- Plain HTML/Tailwind/static/server template: `references/uix/stacks/plain-html.md`.
- Native mobile/React Native/Flutter/Swift/Kotlin: `references/uix/stacks/native-mobile.md`.
- Three.js/WebGL/canvas: `references/uix/stacks/threejs.md`.
- Svelte/SvelteKit: `references/uix/stacks/svelte.md`.
- UniApp/mini-app: `references/uix/stacks/uniapp.md`.

## Verification

- Run focused build/type/lint tests available in the repo.
- Render the changed screen when possible.
- Include only implementation evidence in TaskResult; do not put framework setup notes in product UI.
