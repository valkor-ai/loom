# UIX Focus: Data

Load this when the frontend shows lists, tables, details, charts, logs, metrics, account records, transactions, or query results.

## Data Surface Rules

- Show the data object, status, and available action in the same scan path.
- Provide filters/search/sort/pagination when data volume requires them.
- Preserve current filters and selected record after mutations.
- Use tabular numbers for aligned numeric columns.
- Show units, currency, dates, and timestamps explicitly.
- Distinguish stale, pending, failed, blocked, and completed states.

## Tables And Lists

- Tables need headers, row identity, overflow behavior, empty/loading/error states, and pagination or virtual/infinite loading when relevant.
- Lists/cards need enough metadata to support selection without opening every item.
- Detail views need source context: selected record id/name/status and back/close route.
- Row actions should not be hidden behind hover only.

## Charts And Metrics

- Charts need labels, legends, axes or equivalent context, empty/loading/error states, and accessible summaries when practical.
- Metrics must include context: period, unit, comparison, or source.
- Avoid decorative charts that do not support a user decision.

## Business Feedback

- Business-blocking rules should attach to the affected row, detail, or action region.
- Technical errors should be recoverable and separate from domain restrictions.
