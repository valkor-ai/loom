# React Native Collection Performance

Apply this reference when the task explicitly owns measurable mobile collection performance, virtualization, large feeds/tables, image-heavy rows, refresh/pagination responsiveness, or collection memory pressure.

## Choose The Collection Primitive

Use `FlatList` for dynamic flat collections, `SectionList` for grouped/sticky sections, and an established high-performance list such as FlashList only when the repository already supports it or measurement justifies adoption.

A `ScrollView` with mapped rows is acceptable for a small bounded collection whose full content must render together. Do not replace it mechanically; choose according to expected size, nested scrolling, accessibility, and interaction needs.

Name the representative item count, row complexity, update frequency, image size, and target interaction before tuning.

## Identity And Row Commands

Use stable domain identity in `keyExtractor`. Index keys are unsafe for inserted, removed, sorted, filtered, paginated, refreshed, or optimistic records.

Bind press/swipe/menu/selection actions to the rendered item's stable ID or immutable snapshot. Do not read a mutable selected record after async confirmation.

Preserve row-local draft/focus/expanded state across unrelated updates; if state should reset when identity changes, make that key transition explicit.

## Render Boundaries

Memoize a non-trivial row only after establishing parent churn and stable props. A memoized row still rerenders if callbacks, style objects, selectors, or item objects change each time.

Stabilize `renderItem`, handlers, headers/footers, separators, and `extraData` when they participate in the measured boundary. Do not hide behavior-affecting state from `extraData` or a comparator to reduce render counts.

Select narrow state/query slices per row and avoid each row subscribing to the entire collection/store.

## Measurement And Windowing

Use `getItemLayout` only for truly fixed dimensions including separators. Incorrect estimates break scroll-to-index, focus, sticky headers, and visible-position restoration.

Tune initial render, batch size, window size, clipping, and estimated item size against actual devices and rows. Defaults are often safer than copied aggressive values; Android/iOS clipping and memory behavior differ.

For variable rows, use the list library's measurement/override mechanisms and stable item types instead of pretending all rows share one height.

## Refresh, Pagination, And Search

Separate initial loading, pull-to-refresh, loading-more, empty, end-of-list, partial/error, offline, and retry state. Refresh should not erase usable content unless the product contract requires it.

Guard duplicate pagination with request/cursor ownership, not only a stale `loading` closure. Use server cursors/tokens when provided and reject responses for superseded filters/accounts.

Deduplicate by stable identity, preserve ordering rules, and define how refresh reconciles optimistic/local rows. `onEndReached` may fire more than once and during layout changes.

## Images And Memory

Request/display appropriately sized images, reserve dimensions, provide failure fallback, and use the repository caching component. Avoid decoding full-resolution images in many rows.

Dispose row-owned resources and avoid retaining large item histories, closures, or decoded assets after data changes.

## Accessibility And Interaction

Give rows and actions accessible names, roles, states, and adequate touch targets. Avoid nesting multiple ambiguous pressables without clear focus/activation behavior.

Preserve keyboard/screen-reader focus when rows recycle, filters change, or pagination appends. Announce refresh/error/end state where product behavior requires it.

## Verification

- Profile a production-like build with representative data on the affected platform/device class.
- Verify keys and exact action targets across sort, filter, refresh, pagination, optimistic updates, and row recycling.
- Exercise initial/refresh/more/empty/end/error/offline states and duplicate `onEndReached` calls.
- Test scroll-to-index/restoration and dynamic dimensions when layout optimization is used.
- Check memory/image behavior, focus, touch targets, and accessible row/action semantics.

## Delivery Evidence

Report the workload, primitive, identity strategy, measured bottleneck, tuning decision, before/after observation, and correctness assertions. `memo`/`useCallback` presence or a smooth tiny fixture is not performance evidence.

## Unsafe Defaults

- FlatList required for every small bounded collection.
- Every row memoized and every callback wrapped without measurement.
- Index keys or selected global state used for row commands.
- Fixed `getItemLayout` copied for variable rows.
- Aggressive window/clipping values copied across platforms.
- Pagination guarded only by a stale local boolean.
- Performance gains claimed from development mode or unrealistic data.
