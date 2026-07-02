# UIX Stack: React

Use for React, Next.js, Remix, Vite React, and similar component-driven React projects.

## Structure

- Follow the repo's existing router and component conventions.
- Keep app shell, page route, feature components, and reusable UI primitives separated once the screen has real workflow complexity.
- Prefer feature folders for business surfaces and `components/ui` or existing design-system folders for reusable primitives.
- Keep data fetching/mutation logic close to route/page conventions, but avoid putting every state and helper into one giant component.

## Suggested Component Split

```text
src/
  app|pages|routes/
    feature-page/
  components/
    layout/
    ui/
    feature-name/
  lib|services/
    api-client
    formatters
  styles/
    tokens
```

For a workbench page, split at least:

- `AppShell` or route layout.
- `PageHeader` or topbar.
- `FilterBar`.
- `DataTable` or object list.
- `DetailPanel` or drawer.
- `FormPanel` for create/edit actions.
- `EmptyState`, `ErrorState`, and `SkeletonRows`.

## Implementation Rules

- Use semantic tokens through CSS variables, Tailwind theme, CSS modules, styled system, or the repo's existing approach.
- Represent UI states explicitly: loading, empty, error, validation, submitting, success, disabled, and business-blocking.
- Use controlled form state or a form library already present in the repo.
- Use stable keys, memoization only where useful, and avoid state that can drift between displayed record and submitted record.
- Use existing icon libraries; prefer accessible icon buttons with labels/tooltips.
- If `designTokenAssetPlan.templateId` is selected, adapt the template into the repo's existing global CSS/Tailwind/theme location. Do not paste template declarations into every component.
- Keep route/page components as orchestration and move repeated UI into feature/shared components when the page owns more than one workflow region.

## State Pattern

```tsx
type LoadState<T> =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'empty' }
  | { status: 'ready'; data: T };
```

Do not collapse business blocking into generic `error`. Keep it as a domain state rendered near the related action.

## React/Next Notes

- Next App Router: keep server/client boundaries clear; only mark components client-side when they need interaction.
- Vite/SPA: keep API clients and state helpers outside the page component when reused.
- Avoid hydration mismatch from random values, dates, or viewport-only rendering.
- Put metadata and runtime/deployment notes in docs or results, not product UI.

## Verification

- Run the repo's focused build/lint/test commands when available.
- Render the page and inspect at relevant viewport sizes.
- Check that state transitions do not remount the whole surface unnecessarily.
- Confirm `frontendQualitySelfCheck.designTokenEvidence.tokenConsumerFiles` includes React components or styles that actually consume the token asset.
