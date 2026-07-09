# UIX Scenario: Admin Dashboard

Use for internal operations, staff consoles, CRM/ERP/CMS back offices, SaaS control panels, and management tools. Efficiency, scanability, and workflow completion matter more than visual spectacle.

## Baseline

- First viewport is the working console, not a landing page.
- Typical shell: sidebar navigation, topbar with page context/search/actions, main content, optional right detail drawer.
- Density is usually `workbench_dense` or `balanced`.
- Visual style is restrained: quiet surfaces, strong information hierarchy, semantic status colors, and predictable controls.

## Brief Extraction

When this scenario is selected, map scenario rules into the task brief this way:

| Brief area | Admin dashboard extraction |
| --- | --- |
| `layoutContract` | Sidebar/topbar/main/detail regions, workbench density, desktop split layout, tablet rail/drawer, mobile list-to-detail fallback. |
| `informationContract` | Record identity, status, key decision fields, filters/search/sort/pagination, selected-detail summary, update/history context. |
| `actionContract` | Primary create/submit/approve action near the working region; row/detail contextual actions; destructive actions with confirmation or recovery. |
| `stateContract` | Results-region loading/empty/error, form validation, action pending/success, and business-blocking near row/detail/action. |
| `visualContract` | Restrained operational surfaces, tokenized spacing/type/status colors, compact app identity, no hero/marketing/filler sections. |
| `contentBoundary` | Product copy only: labels, status, validation, filters, actions, and help route. No runtime, delivery, stack, or verification language. |

## Required Patterns

- Navigation: active section, grouped nav items, stable page title, breadcrumbs when depth is greater than one.
- Data: table/list with filters, search, sort, pagination or infinite scroll, empty/loading/error states.
- Detail: row selection opens side panel, route detail, or inline expansion without losing list context.
- Forms: grouped sections, field validation, disabled/submitting state, business-blocking feedback.
- Actions: primary action visible near the relevant region; destructive actions require confirmation or recovery.
- Feedback: success updates affected row/detail and appears near the changed object.

## Layout

- Desktop: sidebar 220-280px, topbar 52-64px, content region with min-width handling.
- Tablet: sidebar collapses to drawer or rail; filters may move into drawer.
- Mobile: convert table-heavy views to list/detail cards or full-screen detail routes; do not depend on hover.
- Use sticky table headers or action bars only when they do not hide content.

## Concrete Shell Pattern

Use this as a structural pattern, not as required class names:

```css
.admin-shell {
  min-height: 100dvh;
  display: grid;
  grid-template-columns: 240px minmax(0, 1fr);
  background: var(--surface);
  color: var(--text);
}

.admin-shell.has-detail {
  grid-template-columns: 240px minmax(0, 1fr) minmax(320px, 380px);
}

.admin-sidebar {
  position: sticky;
  top: 0;
  height: 100dvh;
  border-right: 1px solid var(--border);
  background: var(--surface-raised);
  overflow-y: auto;
}

.admin-main {
  min-width: 0;
  display: grid;
  grid-template-rows: 56px minmax(0, 1fr);
}

.admin-content {
  min-width: 0;
  padding: var(--space-6);
  overflow: auto;
}
```

```css
@media (max-width: 1023px) {
  .admin-shell,
  .admin-shell.has-detail {
    grid-template-columns: minmax(0, 1fr);
  }

  .admin-sidebar {
    position: fixed;
    z-index: var(--z-modal);
    inset: 0 auto 0 0;
    width: min(280px, 86vw);
    transform: translateX(-100%);
  }

  .admin-sidebar[data-open="true"] {
    transform: translateX(0);
  }
}

@media (max-width: 767px) {
  .admin-main {
    grid-template-rows: auto minmax(0, 1fr);
  }

  .admin-content {
    padding: var(--space-4);
  }
}
```

## Component Anatomy

An admin page usually needs these regions in this order:

```html
<aside data-region="sidebar">
  <header data-region="brand"></header>
  <nav aria-label="Primary"></nav>
  <footer data-region="workspace"></footer>
</aside>

<main data-region="main">
  <header data-region="topbar">
    <nav aria-label="Breadcrumb"></nav>
    <div data-region="global-search"></div>
    <div data-region="page-actions"></div>
  </header>

  <section data-region="page-heading"></section>
  <section data-region="filters"></section>
  <section data-region="results"></section>
</main>

<aside data-region="detail-panel"></aside>
```

Do not render this as visible explanatory text. It is a structure guide for implementation.

## Data Table Pattern

- Toolbar left: search, filters, saved views, refresh when relevant.
- Toolbar right: primary create/import action, export only when useful, column settings only for dense data.
- Table header: sortable columns only when sorting is implemented.
- Row: identity, status, key fields, last update, contextual actions.
- Footer: pagination, selected-count actions, or continuation token.
- Detail panel: selected record summary, status, eligible actions, history/events, and close/back control.

```css
.admin-table-wrap {
  min-width: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
  overflow: hidden;
}

.admin-table-scroll {
  overflow: auto;
}

.admin-table {
  width: 100%;
  min-width: 760px;
  border-collapse: collapse;
}

.admin-table th,
.admin-table td {
  height: 44px;
  padding: 0 var(--space-3);
  border-bottom: 1px solid var(--border);
  text-align: left;
  vertical-align: middle;
}
```

On mobile, do not simply shrink this table. Use record cards or a drill-down list when the workflow is record management rather than spreadsheet comparison.

## States

- Loading: table skeleton or row skeleton, not a full-page spinner after shell loads.
- Empty: explain the business reason and next action.
- Error: show retry and preserve filters/form inputs.
- Business-blocking: message must reference the rule and affected object.

## State Placement

- List loading belongs inside the results region, while the shell remains usable.
- Detail loading belongs inside the detail panel, not over the whole page.
- Empty state replaces the result rows and keeps filters visible.
- Business-blocking messages appear next to the affected action and in the detail panel when the action target is a selected record.
- Toasts can confirm success, but the row/detail state must also update.

## Visual Density

- Use compact but readable rows: 40-48px row height for desktop workbench views.
- Use 13-14px table text and 15-16px form/control text unless the existing system differs.
- Use borders and subtle surface contrast before heavy shadows.
- Reserve strong color for primary action, active navigation, focus, and semantic status.
- KPI cards are allowed only when they support the page task; avoid decorative hero metrics.

## Production Criteria

- The first viewport must contain working navigation, current page context, and at least one real work region such as table/list/form/detail/action panel.
- App identity stays compact in the sidebar or topbar. Do not add a large brand intro block above the work surface.
- Header/footer text must be operational: filters, status, user/workspace, primary action, pagination, or help route. Long feature descriptions are not part of a repeat-use console.
- Mobile fallback must keep the same workflow reachable: list/search first, then detail/action through drawer, card, or route.
- Evidence should name the admin shell, data/list surface, and mutation/detail surfaces when they were touched.

## Avoid

- Hero sections, marketing footers, feature-explainer cards, decorative metrics, or long implementation notes.
- Modal-only workflows that make users lose list context.
- Tables without overflow, pagination, or responsive fallback.

## Quality Gate Index

| Gate | Pass signal | Fail signal |
| --- | --- | --- |
| `admin.shell.work_surface` | First viewport contains compact app identity, navigation/current context, a real table/list/form/detail/action region, and primary business action access. | Page opens with hero copy, large intro, footer-like explanation, decorative metrics, or no immediately usable work surface. |
| `admin.topbar.context_actions` | Topbar/header carries operational context such as current page, search/filter, user/workspace, or relevant command. | Header is filler copy, product explanation, or detached from the active task. |
| `admin.list.filter_table_detail` | Record screens preserve list context across filters, pagination, selection, detail, and mutation feedback. | Selection or mutation loses context, table lacks state handling, or every action is isolated in generic modals without row/detail readback. |
