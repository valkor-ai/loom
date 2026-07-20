# React Performance

Optimize only a task-owned, measurable rendering, interaction, startup, bundle, or memory risk. Begin with state ownership and component boundaries; memoization is a later tool, not the definition of performance work.

## Establish The Constraint

Name the affected interaction and the representative workload: row count, update frequency, route chunk, image set, chart size, input latency, or retained resource. Reproduce with production-like data and a production build when development Strict Mode or source transforms distort measurements.

Use the repository's existing profiler, bundle analyzer, performance test, or browser tooling. Do not add a permanent dependency solely to produce one measurement when built-in timing and React DevTools answer the question.

Define a comparison that can be repeated. A lower render count is not useful if the visible workflow becomes stale or inaccessible.

## State Locality And Render Boundaries

Place transient state at the smallest owner that needs it. Typing in a filter, opening a row menu, or editing one form should not require unrelated page regions to subscribe to every update.

Select narrow store/query slices and preserve referential stability where consumers rely on it. Avoid one context value containing frequently changing data plus unrelated actions; split by lifetime or concern when profiling shows broad invalidation.

Use stable domain keys. Index keys and remounting component definitions can turn updates into teardown/recreation, lose focus, and invalidate local state.

## Memoization Decisions

Use `memo` when a meaningful child is repeatedly rendered with equivalent props. Keep comparison functions complete and cheaper than rendering; compare all behavior-affecting props rather than only an ID.

Use `useMemo` for expensive derivation or an identity required by a memoized/subscription consumer. Use `useCallback` when callback identity is part of a proven boundary. Inline closures are acceptable outside measured hot paths.

Memoization does not repair incorrect dependencies. Stale callbacks, validators, permissions, locale, or selected targets are correctness defects even when a profile is faster.

## Collections And Expensive Work

Filter, sort, group, and aggregate once at the owning boundary. Keep source collections immutable and avoid repeating equivalent work in each row.

For large collections, choose pagination, incremental rendering, or virtualization according to product behavior. Virtualized rows require stable item identity, measured/estimated size handling, keyboard/focus behavior, accessible collection semantics, and correct scroll restoration.

Move CPU-heavy pure work off the urgent interaction path only when measurement justifies worker/chunking complexity. Preserve cancellation and stale-result ordering.

## Responsiveness And Scheduling

Use `useTransition` or deferred values for non-urgent rendering while urgent input remains responsive. Pending UI must still expose the committed versus requested state and must not submit stale filters or targets.

Debounce network or expensive query work according to business behavior, not render updates indiscriminately. Cancel pending work and define what happens when input changes rapidly.

## Bundle And Asset Cost

Split route-level or genuinely heavy optional capabilities with the repository's router/framework mechanism. Give lazy boundaries stable loading and error behavior; avoid a spinner flash for tiny local components.

Import library subpaths only when supported, remove duplicate dependencies, and keep server-only or optional packages out of the client bundle. Check generated chunks rather than assuming a dynamic import guarantees useful separation.

Optimize images through the established asset pipeline with dimensions, responsive sources, lazy/eager priority, and layout stability appropriate to the surface.

## Resource Lifetime

Dispose observers, subscriptions, workers, object URLs, timers, and third-party widgets. Bound caches and retained histories. Performance work that only reduces renders while leaking browser resources is incomplete.

## Verification

- Capture before/after measurements for the named workload and interaction.
- Prove memoized paths update when every behavior-affecting prop changes.
- Exercise large collection identity, action targeting, focus, scrolling, empty/loading/error states, and responsive layout.
- Verify lazy chunks through the production build and exercise loading/error recovery.
- Test rapid input, interruption, cancellation, and stale-result prevention for scheduled work.
- Re-run accessibility and business-state checks after virtualization or rendering changes.

## Delivery Evidence

Report the bottleneck, representative workload, selected intervention, repeatable measurement, and visible correctness assertions. A bundle build, profiler screenshot without context, or blanket memoization count does not establish improvement.

## Unsafe Defaults

- `memo`, `useMemo`, or `useCallback` applied to every component/value.
- Custom comparators that ignore callbacks, permissions, locale, or mutable objects.
- Virtualization introduced without focus, semantics, or dynamic-size behavior.
- Development-only timing presented as production evidence.
- Code splitting without testing chunk loading and failure behavior.
- Transition/debounce logic allowed to submit stale targets or hide pending state.
