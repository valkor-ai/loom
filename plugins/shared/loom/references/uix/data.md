# Data UIX

Load this file for dashboards, tables, analytics, finance, research, charts, maps, metrics, reports, or visualization-heavy screens.

## Chart Selection

- Choose the simplest view that communicates the decision: bars for categorical comparison, lines for trends, scatter for relationships, histogram or box plot for distribution, heat map for dense matrix patterns.
- Avoid pie or donut charts except for a few categories where exact comparison is not critical.
- Use tables when users need lookup, audit, export, exact values, or dense comparison.
- For mobile, consider summary cards, prioritized tables, or drill-down views instead of shrinking full desktop charts.

## Encoding

- Keep color encoding consistent across related views.
- Use sequential scales for ordered values, diverging scales for above/below midpoint, and categorical palettes for unrelated groups.
- Do not rely on color alone. Add labels, patterns, shapes, annotations, or direct values where needed.
- Start bar-chart axes at zero unless there is a clear, labeled reason not to.

## Interaction

- Interactive charts need keyboard-accessible controls, visible focus, touch-friendly targets, and usable tooltip alternatives.
- Filters, date ranges, legends, and drill-downs should show applied state and provide a clear reset path.
- Loading, empty, stale, partial-data, and error states must be visible for every data surface.
- For real-time or finance-like views, show refresh timing, data source, units, and stale-data warnings when relevant.

## Accessibility And Trust

- Provide text alternatives or summaries for charts that carry important information.
- Label axes, units, currency, date ranges, sample size, benchmark, or target clearly.
- Do not decorate data in ways that obscure comparison or imply false precision.
- Test with realistic data, outliers, zero values, missing data, long labels, and dense series.

## Evidence

- Record chart/table choices, data states covered, responsive behavior, accessibility alternatives, and any manual-review assumptions in TaskResult.
