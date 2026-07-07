# API Pagination And Collection Reads

## When Pagination Is Required

Pagination is required when a collection endpoint can grow beyond a small bounded current-phase dataset or when the UI supports repeated list scanning/searching.

It is not required for fixed lookup lists, enum-like values, or small bounded lists explicitly constrained by the requirement.

## Strategy Selection

| Strategy | Use When | Avoid When |
|---|---|---|
| Page/size | Internal tools, admin tables, simple random access. | Very large or fast-changing feeds. |
| Offset/limit | Existing APIs already use offset. | High offset performance matters. |
| Cursor | Large or changing datasets, infinite scroll, timeline feeds. | Users need direct page numbers. |
| Keyset | Stable indexed sort fields exist. | Sorting is arbitrary or multi-field without index support. |

## Response Shape Choices

Declare response metadata according to the selected strategy:

| Shape | Use When |
|---|---|
| `items`, `page`, `size`, `totalElements`, `totalPages` | Page/size admin tables where total count is affordable and useful. |
| `items`, `offset`, `limit`, `hasMore` | Offset/limit APIs where total count is optional or expensive. |
| `items`, `nextCursor`, `hasMore` | Cursor/keyset APIs or changing datasets. |
| Link headers or `_links` | Existing repository/API convention already uses navigational links. |

Total count is optional. Do not force expensive `COUNT(*)` behavior for large or rapidly changing datasets. If the UI needs total pages, state the cost and verification expectation.

## Contract Fields

An AAC `http_api` interface for a paginated collection should include:

- `paginationPolicy.strategy`
- `paginationPolicy.requestFields`
- `paginationPolicy.responseFields`
- `paginationPolicy.defaultLimit`
- `paginationPolicy.maxLimit`
- `filterFields` and `sortFields` when applicable
- whether total count is returned, omitted, or client-requested

Example:

```json
{
  "paginationPolicy": {
    "strategy": "page_size",
    "requestFields": ["page", "size"],
    "responseFields": ["items", "page", "size", "totalElements", "totalPages"],
    "defaultLimit": 20,
    "maxLimit": 100
  }
}
```

## Filter And Sort Semantics

Filtering must be applied before pagination. Sorting must be stable enough that repeated page reads do not skip or duplicate records.

For cursor or keyset pagination, the cursor should encode the stable sort fields or the interface should declare the fields that form the cursor. For ordinary admin tables, page/size with a stable default sort is usually sufficient.

## Edge Cases

Collection contracts should state the behavior for:

- empty result sets
- last page
- out-of-range page or offset
- invalid cursor
- page size above maximum

Use existing project conventions for whether out-of-range pages return an empty page or a client error. Do not leave this behavior for the frontend to guess.

## Verification Hooks

For collection tasks, prefer tests or runtime checks that prove:

- default pagination returns a bounded page
- filters/sorts used by the UI are wired to backend query behavior
- response shape contains the fields the frontend uses
- empty or out-of-range behavior matches the declared policy

Do not add broad pagination infrastructure to write-only or detail-only tasks.
