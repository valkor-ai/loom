# Flutter Performance Quality

This file applies Flutter performance discipline to task-owned rebuild behavior, lists, images, animations, expensive computation, memory, and profiling-sensitive UI paths.

## When To Use

- The task creates or changes large lists, image-heavy screens, animated surfaces, complex widgets, performance fixes, rebuild optimization, expensive data transforms, or platform/profile behavior.
- Use this when UI responsiveness, frame time, memory usage, scroll smoothness, or app-start/rendering cost is part of the task risk.
- Pair with widget, Riverpod, Bloc, or navigation references when performance depends on state selection or route structure.

## Implementation Focus

- Add `const` to static widgets and constructors. Avoid rebuilding static child trees, padding, icons, labels, and decorations.
- Limit rebuild scope. Use Riverpod `select`, Bloc `buildWhen`, smaller widgets, localized consumers/builders, or provider boundaries so unrelated state changes do not rebuild whole screens.
- Use lazy lists for dynamic collections: `ListView.builder`, `GridView.builder`, slivers, or existing virtualized abstractions. Avoid eager child arrays for large or unknown-size data.
- Use stable keys for list rows and animated children so Flutter can preserve element identity across filtering, sorting, pagination, and refresh.
- Use `RepaintBoundary` for genuinely expensive repaint regions, not as a blanket wrapper around every widget.
- Optimize images with caching, explicit dimensions, memory resize, placeholders, and error states according to repository dependencies.
- Move heavy synchronous computation off the UI isolate with `compute()` or repository background work patterns.
- Dispose controllers, animation controllers, focus nodes, streams, subscriptions, and timers. Memory leaks often appear as performance issues after navigation.
- Measure when the task is performance-motivated. Do not claim performance improvement solely from code shape.

## Verification Focus

- Run `flutter analyze` and focused tests; run profile/devtools checks when the task explicitly fixes jank, scroll performance, image performance, or startup cost.
- Verify scroll smoothness, pagination, refresh, image loading, animation, and interaction responsiveness on representative data.
- Verify no broad rebuilds after unrelated state changes when selectors/buildWhen/localized builders are part of the fix.
- Verify heavy computation does not block typing, scrolling, route transitions, or animations.
- Verify controllers/listeners are disposed by navigating away or closing affected screens in tests or manual checks.

## Evidence Focus

- In the evidence summary, name the performance decision: const optimization, rebuild boundary, lazy list, key strategy, RepaintBoundary, image optimization, isolate compute, disposal proof, or profile evidence.
