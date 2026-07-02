# UIX Focus: Frameworks

Load this when choosing how to apply a UI contract in a specific frontend stack. For MCP-driven work, prefer stack references from `uiQualityContract.referenceProfile.referenceIds`.

## Rule

Frameworks implement the UI contract; they do not redefine the product surface. Keep scenario, quality level, states, and forbidden user-visible content aligned with the MCP artifact.

## Component Architecture

- Use existing routing, data fetching, component, styling, and test conventions.
- Separate app shell, feature screens, shared primitives, API/data modules, and utilities.
- Avoid one giant component once the workflow includes table/list, form, modal/drawer, and async states.
- Keep design tokens in the framework's normal location: CSS variables, Tailwind config, theme provider, app CSS, or native theme.
- If the repo has a component library, wrap or configure it before hand-writing competing primitives.
- Keep server/client boundaries explicit in SSR frameworks; UI state and browser APIs belong on the client side.

## Minimum Split For Real Screens

```text
layout shell
page route/view
feature components
shared UI primitives
data/API module
formatters/validators
state-specific components
```

If a task creates table/list + detail + form/action in one surface, it should not remain as one large component unless the existing project explicitly uses that pattern for small screens.

## Existing Style Adoption

Before applying a token template:

```text
1. Locate existing theme/tokens/global styles.
2. Decide reuse_existing, extend_existing, or create_* from designTokenAssetPlan.
3. Register the token asset once.
4. Move repeated raw values into semantic roles.
5. Keep component classes/props consistent with the selected stack.
```

Do not create both `tokens.css` and a separate Tailwind theme for the same project unless the repo already uses both and they are linked intentionally.

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
- Check that framework-specific client/server boundaries, hydration, routing, and state ownership do not break the UI contract.
- In `frontendQualitySelfCheck`, name the files that consume the declared token asset, not only the file that defines tokens.
