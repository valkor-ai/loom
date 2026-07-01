# UIX Scenario: Data Console

Use for analytics, monitoring, reporting, logs, query/results pages, operational metrics, and data-heavy workbenches.

## Baseline

- First viewport shows data controls and results, not explanatory content.
- Density is usually `workbench_dense`.
- Layout prioritizes scanning, comparison, filtering, and drill-down.
- Color must separate data series, health/status, and interaction state without overloading one palette.

## Required Patterns

- Filter/search/query controls with visible current criteria.
- Results region with loading, empty, partial, error, and stale-data states.
- Table/grid/chart area with explicit overflow behavior.
- Detail drill-down via side panel, drawer, tabs, or route.
- Timestamp, refresh, or data freshness indicator when data changes over time.
- Export/copy/share actions only when relevant and safe.

## Layout

- Desktop: filter bar or left filter panel, main data region, optional right inspector.
- Wide screens may use split panes, but each pane needs min-width and scroll rules.
- Mobile: reduce chart/table complexity, provide summary cards and drill-down.
- Logs/code/data grids need monospace/tabular numeric treatment when alignment matters.

## States

- Loading should preserve chart/table dimensions.
- Empty state should distinguish "no data yet" from "filters returned no results".
- Error state should separate query/system failure from permission/business restrictions.
- Long labels and values must remain readable through truncation, wrapping, or detail reveal.

## Avoid

- Dashboard wallpaper: many charts with no decision workflow.
- Single giant KPI cards without underlying data access.
- Unlabeled color legends or status colors without text.
