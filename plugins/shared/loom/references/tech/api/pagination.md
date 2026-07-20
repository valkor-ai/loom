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

A paginated collection contract should include:

- `paginationPolicy.strategy`
- `paginationPolicy.requestFields`
- `paginationPolicy.responseFields`
- `paginationPolicy.defaultLimit`
- `paginationPolicy.maxLimit`
- `filterFields` and `sortFields` when applicable
- whether total count is returned, omitted, or client-requested

For example, a page/size admin table should name the `page` and `size` request fields, the collection and metadata response fields, a default page size, and a maximum page size.

## Filter And Sort Semantics

Filtering must be applied before pagination. Sorting must be stable enough that repeated page reads do not skip or duplicate records.

For cursor or keyset pagination, the cursor should encode the stable sort fields or the interface should declare the fields that form the cursor. For ordinary admin tables, page/size with a stable default sort is usually sufficient.

Allow only declared filter and sort fields. Reject or consistently ignore unknown fields according to the existing API contract; never pass client-provided field names or operators directly into SQL.

## Cursor And Keyset Contract

- Treat cursors as opaque client tokens. Do not require clients to construct or modify their internal representation.
- Include every effective sort value, sort direction, and a unique tie-breaker in the cursor or keyset boundary so equal primary sort values cannot skip or duplicate records.
- Do not place secrets or unnecessary personal data in an encoded cursor. Sign or otherwise validate cursors when tampering could expose data or bypass a filter boundary.
- Define cursor version or expiry behavior only when the server may change the encoding or the result window has a real lifetime.
- For bidirectional navigation, define separate next/previous semantics and reverse both comparison and ordering correctly before restoring response order.
- Keep filters and sort policy stable across cursor requests. A cursor created for one filter/sort combination must not silently continue a different query.

## Limits And Count Cost

- Declare minimum, default, and maximum page size when the selected strategy accepts a client-controlled size.
- Define the response for negative values, zero where unsupported, values above the maximum, malformed cursors, and out-of-range pages.
- Return total count only when a client workflow needs it and the query cost is acceptable. `hasMore` or a next cursor is sufficient for many feeds and large datasets.
- Align keyset fields and common filter/sort combinations with the persistence query and index design owned by implementation tasks.

## Edge Cases

Collection contracts should state the behavior for:

- empty result sets
- last page
- out-of-range page or offset
- invalid cursor
- cursor/filter or cursor/sort mismatch
- page size above maximum

Use existing project conventions for whether out-of-range pages return an empty page or a client error. Do not leave this behavior for the frontend to guess.

## Verification Hooks

For collection tasks, prefer tests or runtime checks that prove:

- default pagination returns a bounded page
- filters/sorts used by the UI are wired to backend query behavior
- response shape contains the fields the frontend uses
- repeated reads with equal sort values do not skip or duplicate records
- malformed or tampered cursors follow the declared error behavior
- empty or out-of-range behavior matches the declared policy

Do not add broad pagination infrastructure to write-only or detail-only tasks.
