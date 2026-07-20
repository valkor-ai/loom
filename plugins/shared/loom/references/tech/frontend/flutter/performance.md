# Flutter Performance And Profiling

Apply this reference only when the task owns a measurable rendering, rebuild, list, image, animation, memory, startup, or interaction-latency risk. Do not attach it to every Flutter screen or claim improvement from code shape alone.

## Measure The Right Mode

Use profile mode on representative target hardware/data; debug mode includes assertions/JIT/tooling overhead and release mode lacks much profiling visibility. Record the affected workflow, device/platform, dataset, frame/build/raster/memory metric, and before/after condition.

Use Flutter DevTools performance/frame chart, rebuild tracking, raster stats, memory/allocation snapshots, network/image diagnostics, and app-size tooling as appropriate. Optimize the measured bottleneck, not every possible micro-pattern.

## Rebuild Scope

Use const/static child extraction, smaller widgets, Riverpod `select`, Bloc `buildWhen`, `ValueListenableBuilder`, or equivalent selected state boundaries to prevent unrelated rebuilds.

Do not move all widgets into functions for "optimization"; widget classes/const identity and clear ownership are usually more useful. Avoid expensive sort/filter/map/date/format/allocation work in `build()`.

Stable keys preserve dynamic element identity, but `GlobalKey` is expensive and should be limited to semantics requiring cross-tree/state access. Do not assign unique/global keys indiscriminately.

## Lists And Scrolling

Use lazy builders/slivers and pagination for large/unknown collections. Bound prefetch, cache extent, keep-alive, and concurrent page loads. Avoid nested scrollables, broad shrinkWrap, eager mapped children, and unbounded `AutomaticKeepAlive`.

Preserve row identity and prevent multiple page requests during rapid scroll. Measure layout/raster and memory with representative item complexity/data volume.

## Paint, Images, And Animation

Use `RepaintBoundary` only around independently repainting expensive regions confirmed by profiling. Too many boundaries consume memory/compositing resources.

Provide image dimensions/fit/placeholders/errors, use appropriately resized/cache-width assets, and follow selected caching policy. Huge source images decoded for tiny thumbnails waste memory/raster time.

Keep animations bounded and avoid rebuilding/painting the full page each tick. Use selected implicit/explicit animation patterns, cached children, and reduced-motion behavior. Profile both UI and raster threads.

## CPU, Isolates, And I/O

Move genuinely heavy pure computation/decoding to `compute`/isolates or native/background boundaries after measurement. Isolate startup/copying has cost and plugins/platform channels may not work in arbitrary isolates.

Do not perform synchronous file/database/crypto/network or large JSON processing on the UI isolate. Batch/debounce work and propagate cancellation/staleness so outdated results do not overwrite current state.

## Memory And Lifecycle

Dispose animation/text/scroll/focus controllers, subscriptions, timers, streams, image resources, and manually owned providers/blocs. Use memory snapshots and repeated navigation to detect retained screens/objects.

Bound caches, provider families, keep-alives, list pages, and image memory. A smooth first render that leaks on every route visit is not a successful optimization.

## Startup And Bundle

Keep startup initialization to required local work; defer noncritical network/analytics/heavy setup with explicit readiness behavior. Avoid duplicate plugin/provider initialization.

Use app-size/deferred loading/tree-shaking tools when binary/web bundle size is task-owned. Do not add/remap dependencies solely for smaller size without platform/functionality verification.

## Verification

- Capture before/after profile evidence for the same target, data, and interaction.
- Verify no broad rebuilds from unrelated state and preserved visible correctness.
- Exercise representative list scrolling, pagination, images, animation, typing, and route transitions.
- Test outdated work cancellation and no UI-isolate blocking for moved computations.
- Repeat navigation/actions while inspecting memory and disposal.
- Run focused tests/analyze to guard behavior while optimizing.

## Delivery Evidence

Record the measured bottleneck, environment, before/after metric/trace, changed boundary, and regression assertions. Added `const`, `RepaintBoundary`, `compute`, or builder APIs alone are not evidence of better performance.

## Unsafe Defaults

- Performance reference selected from generic Flutter work/prose.
- Debug-mode impressions or code shape used as proof.
- `const`, keys, keep-alive, repaint boundaries, or isolates applied everywhere.
- Large eager/shrink-wrapped lists and unbounded page requests.
- Huge images decoded at display-independent source size.
- Controllers/providers/caches retained after route disposal.
