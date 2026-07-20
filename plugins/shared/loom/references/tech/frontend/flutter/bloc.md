# Flutter Shared State With Bloc And Cubit

Apply Bloc/Cubit only when TechnicalBaseline selects Bloc and the task owns shared client state. Do not introduce it for local widget state or into a Riverpod-only architecture.

## Cubit Or Bloc

Use Cubit for command-to-state workflows whose event source and concurrency are straightforward. Use Bloc when explicit event semantics, event transformers/concurrency, multiple producers, or auditable transition modeling add real value.

Events describe user/system facts and commands precisely; states are immutable UI workflow snapshots. Avoid `UpdateEverything`, one state per field, and event payloads that omit the target and depend on a mutable selected record.

```dart
sealed class ApprovalEvent {
  const ApprovalEvent();
}

final class ApprovalRequested extends ApprovalEvent {
  const ApprovalRequested(this.orderId, this.expectedVersion);
  final String orderId;
  final int expectedVersion;
}
```

Use the repository's equality/immutable generation convention (Equatable, Freezed, sealed records/classes). Replace collections/models instead of mutating them in place.

## State Model

Represent initial/loading/empty/ready/refreshing/submitting/success and typed validation/conflict/forbidden/offline/unavailable failure where relevant. Preserve usable previous data during refresh/mutation only when accepted.

Keep domain/persistence entities out of widget state when a stable view model is needed. Do not store `BuildContext`, widgets, controllers, snack bars, or navigation objects in state/bloc.

State transitions should clear stale failure/pending fields at the correct time and reconcile server-returned ID/version/status after writes.

## Event Concurrency And Async Work

Choose event transformers from product behavior: restartable/latest for replaceable searches, droppable for duplicate submit prevention, sequential for ordered writes, concurrent for independent bounded work.

Do not start required side effects without awaiting/tracking them. Handle repository errors into typed states and keep the bloc event stream alive. Propagate cancellation where supported.

Keep external/network/storage operations in injected repositories/use cases. One bloc/cubit operation owns ordering and partial-failure behavior; widgets should not coordinate a second copy.

## Provisioning And Lifetime

Provide blocs at the narrowest stable app/feature/route/tab boundary matching state lifetime. Use `BlocProvider.value` only for an existing instance whose disposal remains owned elsewhere; creating an instance with `.value` can leak it.

Do not create a bloc in `build()` or repeatedly in list rows. Close manually owned blocs/streams and avoid global singleton feature blocs without an app-wide lifecycle requirement.

## Widget Integration And Side Effects

Use `BlocBuilder` for rendering, `BlocListener` for one-shot navigation/dialog/snack/analytics, and `BlocConsumer` only when the same subtree genuinely owns both.

Use `context.watch`/selectors for rendering and `context.read` in callbacks. `buildWhen`/`listenWhen` may narrow work but cannot repair a monolithic state model.

Listeners must fire once on meaningful transitions. Navigation and dialogs belong in listeners/route orchestration, never reducers/events/state or during build.

## Verification

- Test initial state, commands/events, exact emitted sequence, and repository interactions.
- Cover load/empty/refresh, validation/conflict/forbidden/offline, submit success/failure, duplicate prevention, and ordered/concurrent behavior owned by the task.
- Verify immutable updates and returned record/version reconciliation.
- Test provider lifetime/disposal and no duplicate bloc instances after route/tab rebuild.
- Verify builders render task-owned states and listeners fire once for the displayed target.
- Analyze/build generated immutable state code when applicable.

## Delivery Evidence

Identify the Cubit-vs-Bloc choice, event/command, state sequence, concurrency, and listener assertion proving it. A single `blocTest` happy path or widget provider presence cannot prove target identity, error recovery, lifetime, concurrency, or side-effect deduplication.

## Unsafe Defaults

- Bloc loaded/introduced without selected stack and shared-state ownership.
- Bloc chosen for trivial local state solely for uniformity.
- Vague events reading mutable selected state later.
- Mutable state collections or `BuildContext` held in bloc/state.
- Bloc instances created during build or leaked through `BlocProvider.value`.
- Navigation/snack/dialog actions emitted during build or duplicated by listeners.
