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
- If a token template is selected for the task, adapt it into the repo's existing global CSS, Tailwind, or theme location. Do not paste template declarations into every component.
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
- Confirm evidence names React components or styles that actually consume the token asset.

## Quality Gate Index

| Gate | Pass signal | Fail signal |
| --- | --- | --- |
| `react.split.workflow_regions` | React page orchestration is separated from reusable feature components, data/API modules, formatters, state views, and token-consuming styles. | Page component owns all fetching, form, table, modal/drawer, state rendering, and styling once the workflow has multiple regions. |

## Route And Data Boundary

The UIX stack decides how the visible page is composed; the repository's React,
Next.js, router, data, and API references decide how code crosses runtime
boundaries.

```text
route/layout -> page orchestration -> feature view -> shared primitive
                           \\-> query/mutation adapter -> state view -> readback
```

- Keep route/layout responsible for shell and navigation context, page orchestration responsible for task scope, and feature components responsible for visible regions and actions.
- Keep API clients, query keys, serializers, and mutations in the existing data boundary. Do not fetch from every presentational component.
- For Next.js, preserve server/client boundaries, hydration determinism, loading/error/not-found behavior, and direct route refresh. For Vite or a client-only app, preserve the accepted router and same-origin/API configuration.
- Pass stable record identity and explicit action callbacks into row/detail/form components; do not let a stale global selection decide a mutation target.

## Token And State Ownership

```tsx
<FeaturePage>
  <PageHeader />
  <FilterBar value={query} onChange={setQuery} />
  <ResultRegion state={resultState} />
  <DetailPanel record={selectedRecord} onAction={handleAction} />
</FeaturePage>
```

Use one app-level token source and one state owner per concern. A feature can
extend semantic tokens when its product surface needs a new role, but it must
not create component-local color, spacing, or state conventions that conflict
with the shared system.
