# UIX Scenario: Data Console

Use for analytics, monitoring, reporting, logs, query/results pages, operational metrics, and data-heavy workbenches. The UI must help users filter, compare, inspect, and act on data.

## Baseline

- First viewport shows data controls and results, not explanatory content.
- Density is usually `workbench_dense`.
- Layout prioritizes scanning, comparison, filtering, freshness, and drill-down.
- Color separates data series, health/status, and interaction state without overloading one brand color.

## Console Structure

```html
<main data-region="data-console">
  <header data-region="console-header"></header>
  <section data-region="query-bar"></section>
  <section data-region="summary-strip"></section>
  <section data-region="workspace">
    <aside data-region="filters"></aside>
    <section data-region="results"></section>
    <aside data-region="inspector"></aside>
  </section>
</main>
```

```css
.data-console {
  min-height: 100dvh;
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr);
  background: var(--surface);
}

.data-workspace {
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(220px, 280px) minmax(0, 1fr);
  gap: var(--space-4);
  padding: var(--space-4);
}

.data-workspace.has-inspector {
  grid-template-columns: minmax(220px, 280px) minmax(0, 1fr) minmax(320px, 420px);
}

@media (max-width: 1023px) {
  .data-workspace,
  .data-workspace.has-inspector {
    grid-template-columns: minmax(0, 1fr);
  }
}
```

## Required Patterns

- Query/filter controls with visible current criteria and clear reset.
- Results region with loading, empty, partial, error, and stale-data states.
- Table/grid/chart area with explicit overflow behavior and stable height.
- Detail drill-down via side panel, drawer, tabs, or route.
- Timestamp, refresh, or data freshness indicator when data changes over time.
- Export/copy/share actions only when relevant and safe.

## Table And Chart Regions

```css
.console-panel {
  min-width: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}

.results-scroll {
  min-height: 0;
  overflow: auto;
}

.metric-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: var(--space-3);
}
```

- Tables use tabular numbers for values that must align.
- Charts need labels, legends, empty/loading/error states, and readable axes.
- Logs and query output need monospace alignment, wrapping or horizontal scroll, and copy controls.
- Metrics need period/source context; do not show giant numbers without decision context.

## Operational Record Variant

When the data console is actually a CRUD/approval/request workbench, combine data-console scanning with admin-dashboard record actions:

- Query/filter region remains visible.
- Results region owns loading/empty/error/stale states.
- Inspector/detail region owns selected record facts, action eligibility, history, and business-blocking feedback.
- Mutations must update both data row/card and detail summary when both are visible.

```html
<section data-region="results-plus-inspector">
  <section data-region="record-results"></section>
  <aside data-region="record-inspector"></aside>
</section>
```

## States

- Loading preserves chart/table dimensions.
- Empty distinguishes "no data yet" from "filters returned no results".
- Error separates query/system failure from permission/business restrictions.
- Stale data displays last updated time and refresh path.
- Long labels and values remain inspectable through wrapping, truncation with title, or detail reveal.

## Verification Signals

- Filters/search preserve criteria after refresh or mutation.
- Empty state distinguishes no records from no matches.
- Overflow is scoped to table/log/code regions, not the full page.
- Freshness, timestamp, or result count is visible when relevant.

## Avoid

- Dashboard wallpaper: many charts with no decision workflow.
- Single giant KPI cards without underlying data access.
- Unlabeled color legends or status colors without text.
- Replacing the entire console shell with a spinner after initial load.

## Query Lifecycle

Model the query surface as a repeatable lifecycle rather than a static chart
wall:

```text
criteria -> submitted query -> loading -> results or empty -> stale/refresh
-> selected result -> inspector or drill-down
```

- Show the active criteria and a clear reset path after submission.
- Keep the prior results visible, with an explicit loading indication, when a refresh can complete without invalidating them.
- Distinguish no data, no matches, partial results, permission failure, and query failure in both copy and recovery actions.
- Keep the selected result and query context when opening an inspector or route detail.
- When results change after refresh, show the last-updated time or query revision so users can tell which data they are inspecting.

```ts
type QueryState<T> =
  | { status: 'idle'; criteria: Criteria }
  | { status: 'loading'; criteria: Criteria; previous?: T }
  | { status: 'ready'; criteria: Criteria; data: T; updatedAt: string }
  | { status: 'empty'; criteria: Criteria; updatedAt: string }
  | { status: 'error'; criteria: Criteria; message: string; previous?: T };
```

## Result Accessibility

- Tables expose column meaning, row identity, sortable state, and keyboard navigation where interaction exists.
- Charts pair visual encoding with a readable legend, units, time range, and an accessible tabular or textual alternative.
- Logs and query output use a deliberate wrapping or horizontal-scroll boundary; never make the whole application horizontally scroll.
- Long values remain inspectable through wrapping, copy, detail reveal, or a stable title. Truncation must not remove the value needed for a decision.
- Every color-coded status also has text, iconography, or an equivalent non-color signal.
