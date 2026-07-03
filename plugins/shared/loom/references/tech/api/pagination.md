# API Pagination And Collection Reads

Load this file only when `techReferenceProfile.groups.api` includes `pagination`.

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

## Contract Fields

An AAC `http_api` interface for a paginated collection should include:

- `paginationPolicy.strategy`
- `paginationPolicy.requestFields`
- `paginationPolicy.responseFields`
- `paginationPolicy.defaultLimit`
- `paginationPolicy.maxLimit`
- `filterFields` and `sortFields` when applicable

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

## Verification Hooks

For collection tasks, prefer tests or runtime checks that prove:

- default pagination returns a bounded page
- filters/sorts used by the UI are wired to backend query behavior
- response shape contains the fields the frontend uses

Do not add broad pagination infrastructure to write-only or detail-only tasks.
