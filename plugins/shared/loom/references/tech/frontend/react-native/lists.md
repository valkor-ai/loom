# React Native List Quality

This file applies React Native list-performance discipline to task-owned FlatList, SectionList, FlashList, virtualized rows, refresh, pagination, and row actions.

## When To Use

- The task creates or changes mobile lists, feeds, search results, grouped lists, infinite scroll, pull-to-refresh, row actions, selectable rows, or large scrollable collections.
- Use this when list correctness, action targeting, scroll performance, memory use, pagination, or refresh behavior affects the delivered workflow.
- If the list is tiny and static, keep this file lightweight but still use stable keys and accessible row actions.

## Implementation Focus

- Use `FlatList`, `SectionList`, or the repository's virtualized list abstraction for dynamic or large collections. Do not use `ScrollView` with mapped dynamic rows for potentially large lists.
- Use stable domain keys in `keyExtractor`. Never use row indexes for mutable, sorted, filtered, paginated, or refreshable collections.
- Memoize row components when row rendering is non-trivial or the list updates frequently. Keep `renderItem`, `keyExtractor`, row handlers, separators, headers, and footers stable with `useCallback` or existing patterns.
- Use `getItemLayout` only when item height is truly fixed. Wrong fixed measurements create scroll bugs and broken accessibility focus.
- Tune `initialNumToRender`, `maxToRenderPerBatch`, `windowSize`, and `removeClippedSubviews` according to row complexity and platform behavior. Do not copy extreme values blindly.
- Use `FlashList` or the repository's high-performance list library for very large, image-heavy, or highly interactive lists when dependencies already support it.
- Keep refresh and pagination states separate: initial loading, pull refresh, loading more, no more data, empty state, error state, and retry need distinct behavior when visible to users.
- Prevent duplicate pagination calls with loading/hasMore guards. Avoid using stale `data.length` or old filters when loading more.
- Keep row actions bound to the displayed item's stable identity or snapshot so filtering, sorting, or refresh cannot mutate the wrong record.

## Verification Focus

- Verify initial load, empty, populated, pull-refresh, pagination, end-of-list, error, retry, and row-action targeting states touched by the task.
- Verify filtering/sorting/refresh does not break row identity or selected row actions.
- Verify scroll performance on representative data volume when the task owns list performance.
- Verify list cells do not rerender unnecessarily after unrelated state changes when memoization is part of the fix.
- Verify accessibility labels and touch targets for row actions, swipe actions, and list item pressables.

## Evidence Focus

- In the evidence summary, name the list decision: virtualized list choice, key strategy, memo row boundary, pagination guard, refresh state split, fixed layout proof, FlashList adoption, or row-action targeting proof.
