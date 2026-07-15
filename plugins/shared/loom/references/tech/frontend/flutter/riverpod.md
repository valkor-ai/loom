# Flutter Shared State With Riverpod

Apply Riverpod only when TechnicalBaseline selects it and the task owns shared client state. Local ephemeral widget state does not require providers, and a Bloc-selected project should not receive Riverpod patterns.

## Provider Selection

Choose the smallest provider that matches lifecycle and commands:

| Need | Riverpod boundary |
|---|---|
| Stateless dependency/derived value | `Provider` |
| Read-only finite async data | `FutureProvider` |
| Long-lived stream | `StreamProvider` |
| Command-bearing synchronous state | `NotifierProvider` |
| Command-bearing async workflow | `AsyncNotifierProvider` |
| Parameterized stable identity | `.family` with immutable/equatable key |

Do not use `StateProvider` as an unstructured feature store or create providers for every text field when a form/widget owns the draft.

## Notifier And State Contract

Keep state immutable and model workflow outcomes explicitly. Notifier methods represent product commands and coordinate repositories/ports without holding `BuildContext`.

```dart
@riverpod
class OrderDetail extends _$OrderDetail {
  @override
  Future<OrderViewModel> build(String orderId) =>
      ref.watch(orderRepositoryProvider).load(orderId);

  Future<void> approve(int expectedVersion) async {
    final current = await future;
    state = const AsyncLoading<OrderViewModel>().copyWithPrevious(state);
    state = await AsyncValue.guard(() =>
      ref.read(orderRepositoryProvider).approve(current.id, expectedVersion));
  }
}
```

The exact generated API depends on selected Riverpod version. Preserve previous data during mutation only when UX/product behavior accepts stale content.

Do not mutate watched list/map/model instances. Use copy/update methods and reconcile server-returned identity/version/state.

## Reading, Watching, And Side Effects

Use `ref.watch` for rendering/derived dependencies and `ref.read` for event commands. Use `ref.listen` for one-shot navigation/dialog/snack/analytics behavior at a stable widget/provider boundary.

Never call `watch` inside callbacks, dispatch commands during build, or create repeated listeners as widgets rebuild. Listeners must distinguish initial/loading/data/error transitions and avoid duplicate side effects.

Use `select` to narrow rebuilds when a widget needs one stable field. Do not select a newly allocated collection/view model each time and expect rebuild reduction.

## Lifetime, Families, And Invalidation

Use auto-dispose for route/parameter state that should end when unobserved; keepAlive only with explicit freshness/cache/invalidation policy. Unbounded family keys can retain memory/network state.

Family parameters must be stable value keys, not mutable objects or whole DTOs. Invalidate/refresh after mutations, identity/tenant changes, logout, or accepted staleness events.

Avoid cyclic provider dependencies and hidden global provider containers. Use one app `ProviderScope` plus deliberate nested overrides/scopes where ownership requires.

## AsyncValue And Errors

Render `AsyncValue` loading/data/error while preserving distinctions among empty, validation, conflict, forbidden, offline/unavailable, stale, and unexpected errors in feature state/view models.

Refreshing, initial loading, and mutating are different UX states. Do not erase usable data during background refresh or leave submit controls disabled after errors.

Cancellation/disposal must reach repositories/clients where supported. Do not launch untracked futures from notifiers for required business effects.

## Dependencies And Generated Code

Inject repositories, clocks, storage, permissions, and platform adapters through providers and override them in tests. Providers should not read scattered environment/global singletons.

When annotations/generation are selected, keep source and generated files synchronized through the repository's build_runner policy. Do not hand-edit generated providers.

## Verification

- Test provider/notifier initial, loading, data, empty, validation/conflict/forbidden/offline, refresh, mutation, and recovery transitions owned by the task.
- Verify immutable updates and server readback/version reconciliation.
- Test family key/lifetime, auto-dispose, invalidation, logout/tenant clearing, and no duplicate requests/listeners.
- Verify `select`/scoping limits rebuilds only when performance is claimed.
- Override infrastructure with fakes and assert commands hit the displayed target.
- Regenerate/analyze when annotations or generated providers change.

## Delivery Evidence

Identify the provider/notifier command, lifetime, and state-transition assertion proving it. A `ConsumerWidget` rendering one value or generated file presence cannot prove invalidation, disposal, command failure, immutability, or side-effect deduplication.

## Unsafe Defaults

- Riverpod loaded/introduced without selected stack and shared-state ownership.
- `StateProvider` used as a generic feature store.
- Mutable list/model state updated in place.
- `watch` in callbacks or commands/listeners created during build.
- Families keyed by mutable/full DTO objects and kept alive indefinitely.
- Async errors collapsed to generic empty/error text.
- Generated files edited manually or left stale.
