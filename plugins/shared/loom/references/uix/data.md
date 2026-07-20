# UIX Focus: Data

Load this when the frontend shows lists, tables, details, charts, logs, metrics, account records, transactions, or query results.

## Data Surface Rules

- Show the data object, status, and available action in the same scan path.
- Provide filters/search/sort/pagination when data volume requires them.
- Preserve current filters and selected record after mutations.
- Use tabular numbers for aligned numeric columns.
- Show units, currency, dates, and timestamps explicitly.
- Distinguish stale, pending, failed, blocked, and completed states.
- Keep a stable object identity visible during selection, mutation, loading, and error states.
- Use the domain's real identifiers rather than generated demo names when identifiers exist.

## Brief Mapping

When the task brief includes an information contract, use it as the minimum data
surface contract:

- Required fields become visible fields, summaries, table columns, detail facts, or chart labels.
- Scan priority determines visual order before decorative grouping or local component preference.
- Identity fields stay visible during selection, mutation, loading, and error states.
- Status fields stay close to available actions so users can understand eligibility.
- Long-content policy controls wrapping, truncation, drill-down, overflow, or responsive card fallback.
- The selected data views name the exact views this task should implement or preserve; do not add unrelated dashboards or summaries.

## Record Workbench Pattern

Use this for CRUD, approval, operations, case management, and account/order/request workflows:

```html
<main data-region="record-workbench">
  <header data-region="page-context"></header>
  <section data-region="workbench-toolbar"></section>
  <section data-region="workbench-body">
    <section data-region="record-list"></section>
    <aside data-region="record-detail"></aside>
  </section>
</main>
```

The list is for finding and comparing records. The detail panel is for current status, domain fields, related events, and eligible actions. The form or mutation result must update whichever region owns the affected object.

## Tables And Lists

- Tables need headers, row identity, overflow behavior, empty/loading/error states, and pagination or virtual/infinite loading when relevant.
- Lists/cards need enough metadata to support selection without opening every item.
- Detail views need source context: selected record id/name/status and back/close route.
- Row actions should not be hidden behind hover only.

## Table Anatomy

```html
<section data-region="data-surface">
  <header data-region="data-toolbar"></header>
  <div data-region="data-feedback" aria-live="polite"></div>
  <div data-region="data-scroll">
    <table>
      <thead></thead>
      <tbody></tbody>
    </table>
  </div>
  <footer data-region="pagination-or-selection"></footer>
</section>
```

```css
.data-scroll {
  min-width: 0;
  overflow: auto;
}

.data-scroll table {
  width: 100%;
  min-width: 720px;
  border-collapse: collapse;
}

.data-scroll th,
.data-scroll td {
  height: 44px;
  padding: 0 var(--space-3);
  border-bottom: 1px solid var(--border);
}
```

Mobile record-management flows should prefer cards or drill-down details. Use horizontal table scroll only when users must compare columns.

## Record Card Fallback

```html
<article data-region="record-card">
  <header>
    <strong data-region="record-title"></strong>
    <span data-region="record-status"></span>
  </header>
  <dl data-region="record-facts"></dl>
  <footer data-region="record-actions"></footer>
</article>
```

Use cards for narrow screens when the table's purpose is selection or action. Keep compare-heavy grids scrollable and label the scroll region.

## Charts And Metrics

- Charts need labels, legends, axes or equivalent context, empty/loading/error states, and accessible summaries when practical.
- Metrics must include context: period, unit, comparison, or source.
- Avoid decorative charts that do not support a user decision.

## Volume And View Selection

Select the data view from the user's job and expected volume:

| Need | Preferred view |
| --- | --- |
| Find or compare many records | Table with declared fields, sorting, filtering, and bounded paging. |
| Select and act on a small set | List or cards with identity, status, key facts, and row action. |
| Inspect one record | Detail route, drawer, or split view that preserves source context. |
| Observe trends | Chart paired with labels, units, range, comparison, and a readable data summary. |
| Inspect technical output | Bounded log/result surface with copy, search, filtering, and failure state. |

Do not add a chart, KPI grid, export control, or summary panel unless it supports
the accepted user decision. Do not use a card grid to avoid defining table fields
or pagination for a growing collection.

## Query And Readback States

- Show the active filter, sort, range, or query context near the result count.
- Preserve query context when opening detail, changing a record, retrying, or
  returning from a detail route.
- Distinguish no matching results from no records, unavailable data, stale data,
  and permission-limited data.
- After a mutation, update the owning row/detail/metric or explicitly mark the
  readback as pending. A success toast without updated data is incomplete.
- For large or remote data, define paging, continuation, virtualization, or
  bounded loading before implementing the result view.

## Business Feedback

- Business-blocking rules should attach to the affected row, detail, or action region.
- Technical errors should be recoverable and separate from domain restrictions.
- Success should update the row/detail state, not only show a toast.

## Evidence

A completed data UI should cite:

- Data surface files and token consumer files.
- Query/list/detail/action states that were implemented.
- Empty, loading, error, and business-blocking placement.
- Overflow/responsive behavior for dense values and long labels.

## Quality Gate Index

| Gate | Pass signal | Fail signal |
| --- | --- | --- |
| `data.surface.scan_action_path` | The user can scan object identity, status, key fields, and available action in one path, with loading/empty/error/business-blocking states near the affected region. | Data is hidden behind generic cards, status/action are separated from the record, or state feedback appears only as a global toast/message. |
