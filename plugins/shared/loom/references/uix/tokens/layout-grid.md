# UIX Token: Layout Grid

Load this file when defining page layout, app shells, responsive behavior, dashboards, data consoles, marketing pages, docs, or mobile screens.

## Breakpoints

Use mobile-first breakpoints unless the repo already has a system:

- `sm`: 640px.
- `md`: 768px.
- `lg`: 1024px.
- `xl`: 1280px.
- `2xl`: 1536px.

Do not scale font size directly with viewport width. Use breakpoint-specific layout changes and stable type tokens.

## CSS Token Skeleton

```css
:root {
  --container-sm: 640px;
  --container-md: 768px;
  --container-lg: 1024px;
  --container-xl: 1280px;
  --container-prose: 72ch;
  --shell-sidebar-width: 240px;
  --shell-topbar-height: 56px;
  --shell-detail-width: 380px;
  --table-min-width: 760px;
  --drawer-width: min(420px, 92vw);
}

.container {
  width: min(100%, var(--container-xl));
  margin-inline: auto;
  padding-inline: var(--space-4);
}

@media (min-width: 768px) {
  .container { padding-inline: var(--space-6); }
}
```

## Containers

- Workbench/admin: full-width shell with constrained internal panels where needed; avoid marketing-style centered containers.
- Data console: reserve width for tables, filters, logs, charts, and right-side detail panels.
- Docs/prose: readable content width around 60-75 characters plus navigation/TOC.
- Marketing/corporate: controlled max width with intentional full-bleed media when appropriate.
- Mobile: one-column task flow with fixed/sticky action zones only when they do not hide content.

## Grid Patterns

- Sidebar + topbar + content for repeated operational workflows.
- Table/list + detail panel for record management.
- Filter bar + result table + pagination for searchable datasets.
- Split primary/editor preview only when both panes are used continuously.
- Docs shell with left nav, content, and optional right TOC.
- Scene-first layout for 3D/canvas experiences, with controls overlaid or docked without covering the scene.

## Workbench Shell

```css
.workbench-shell {
  min-height: 100dvh;
  display: grid;
  grid-template-columns: var(--shell-sidebar-width) minmax(0, 1fr);
}

.workbench-main {
  min-width: 0;
  display: grid;
  grid-template-rows: var(--shell-topbar-height) minmax(0, 1fr);
}

.workbench-content {
  min-width: 0;
  overflow: auto;
}
```

Use this pattern for operational pages. Marketing/corporate/docs/3D scenarios have their own layout baselines and should not inherit a workbench shell by accident.

## Stable Dimensions

Define stable dimensions for:

- Navigation rails and sidebars.
- Topbars and sticky action bars.
- Table row height and pagination.
- Icon buttons and segmented controls.
- Cards or tiles in fixed-format grids.
- Canvas/media/chart regions.
- Modal/drawer widths and scroll boundaries.

Stable dimensions prevent hover, loading text, validation text, and long labels from resizing the whole interface.

## Responsive Behavior

- Sidebar becomes drawer or bottom navigation on smaller screens.
- Dense tables become horizontally scrollable tables, list/detail cards, or drill-down views based on task needs.
- Detail side panels become drawers or full-screen detail routes on mobile.
- Toolbars wrap predictably; primary action remains visible.
- Charts keep legends readable and avoid cramped axes.
- Fixed side panels become drawers or route details before they squeeze the main workflow below usable width.
- Header, toolbar, and action regions may wrap, but they must not push the primary task below non-functional explanation content.

## Self-Check

- The selected layout baseline matches the product scenario and density.
- Content does not overlap navigation, sticky bars, side panels, or mobile safe areas.
- Wide desktop does not stretch text or forms into unreadable lines.
- The implemented page contains the actual task surface in the first visible viewport for its scenario.
