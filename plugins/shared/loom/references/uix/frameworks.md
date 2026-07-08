# UIX Focus: Frameworks

Use this when framework structure, shared components, routing, data fetching, state ownership, styling, or frontend build conventions affect a user-visible surface.

## Rule

Frameworks implement the product surface; they do not redefine it. Keep scenario fit, density, visible states, and product-boundary rules aligned with the selected UI target.

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
2. Decide reuse existing, extend existing, or create a new token asset from the current UI styling plan.
3. Register the token asset once.
4. Move repeated raw values into semantic roles.
5. Keep component classes/props consistent with the selected stack.
```

Do not create both `tokens.css` and a separate Tailwind theme for the same project unless the repo already uses both and they are linked intentionally.

## Verification

- Run focused build/type/lint tests available in the repo.
- Render the changed screen when possible.
- Keep framework setup notes out of product UI.
- Check that framework-specific client/server boundaries, hydration, routing, and state ownership do not break the current UI requirements.
- Name the files that consume the declared token asset, not only the file that defines tokens.

## Quality Gate Index

| Gate | Pass signal | Fail signal |
| --- | --- | --- |
| `framework.component_structure` | Multi-region screens are split into shell/page, feature components, shared primitives, data/API helpers, and state-specific components following repo conventions. | Real workflow remains one giant component, duplicated primitives/styles spread across pages, or framework setup notes leak into product UI. |
