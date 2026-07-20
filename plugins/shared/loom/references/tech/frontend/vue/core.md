# Vue Reactivity And Composable Delivery

Implement task-owned Vue surfaces within the repository's Vue version, Options/Composition API convention, language policy, router, state/data libraries, SFC tooling, component system, and UI quality contract. Do not convert established patterns incidentally.

## Repository Convention

Use `<script setup>` and Composition API when they are established or the task explicitly owns migration. Preserve a coherent Options API feature when conversion would expand scope or alter behavior.

Confirm Vue/compiler versions and enabled macros before using `defineModel`, reactive props destructure, generic SFC syntax, or other version-specific behavior. External “modern Vue” examples are not compatibility proof.

Keep route/page components as workflow orchestration and extract focused feature components/composables when state, effects, or reuse make the surface hard to inspect.

## Ref, Reactive, And Identity

Use `ref` for primitives, nullable/replaceable objects, template refs, and values whose replacement is meaningful. Use `reactive` for cohesive object state that retains one proxy identity.

Do not destructure a reactive object into plain values. Use `toRefs`, `toRef`, store helpers, or access through the proxy. Avoid replacing a `reactive` object in a way that disconnects consumers.

Use `shallowRef`/`markRaw` for large immutable payloads or third-party instances only when deep tracking is unnecessary and updates explicitly replace/trigger identity.

Keep editable drafts, persisted/API records, selected snapshots, filters, pending operations, and optimistic values separate when their lifetimes differ.

## Derived State And Watchers

Use `computed` for pure derived values and writable computed only for a real controlled transformation. Do not use watchers to keep redundant state synchronized.

Use `watch` when source/old value/timing control matters; use `watchEffect` for a concise effect whose dependencies are intentionally discovered during synchronous execution. Async dependencies accessed after `await` are not auto-tracked as expected.

Choose `flush` timing deliberately for DOM-dependent work and avoid deep watching large objects. Watch a getter or normalized subset instead.

Cancel or invalidate stale async work with watcher cleanup (`onCleanup`/supported cleanup API), `AbortController`, or operation identity so a prior route/filter/record cannot overwrite current state.

## Lifecycle And Effect Scope

Register lifecycle hooks synchronously during setup. Dispose listeners, timers, observers, subscriptions, workers, browser/native integrations, and manually created watchers.

Use `effectScope` only when a composable/plugin creates a group of effects with an independent lifecycle, and expose/perform scope disposal. Component-owned reactive effects normally stop automatically on unmount.

Template refs are nullable before mount and after unmount/conditional removal. Prefer declarative rendering; use DOM/imperative refs only for focus, measurement, or third-party integration.

## Composable Contracts

Extract a composable for reusable stateful behavior or a complex external lifecycle, not just to move code. Accept refs/getters/values according to the repository convention and normalize with supported utilities such as `toValue` where appropriate.

Return a small public contract with readonly state when callers should not mutate it and named commands for transitions. Avoid hidden router, global store, tenant, auth, or broad API behavior in a generic composable.

For async work, expose meaningful idle/loading/refreshing/ready/empty/error/mutating state, retry/cancel semantics, and stable command targets.

## Workflow And UI

Represent task-owned loading, empty, validation, conflict/stale, forbidden, unavailable, submitting, success, disabled, and rollback states near the affected region.

Preserve valid input after rejection, block duplicate submit, and reconcile returned identity/version/status. Product UI must not expose runtime commands, framework explanations, delivery notes, or verification instructions.

Use semantic elements, labels, focus handling, announcements, and repository UIX tokens/components. Vue directives and transitions must preserve keyboard and reduced-motion behavior.

## Verification

- Run focused SFC type/build/lint and component/composable tests supplied by the repository.
- Prove reactive updates after source replacement, destructuring boundaries, route/record changes, and async completion ordering.
- Test watcher cleanup/cancellation, lifecycle disposal, and remount behavior for external resources.
- Exercise owned workflow states, draft preservation, duplicate blocking, stable targets, and final readback.
- Verify semantics, focus, keyboard interaction, long/localized content, and responsive behavior.

## Delivery Evidence

Name the reactivity owner, ref/reactive/computed/watch decision, composable lifecycle, and assertion proving visible behavior. A successful render or passing typecheck does not prove stale-work safety, cleanup, draft integrity, or accessibility.

## Unsafe Defaults

- Composition API or TypeScript migration attached to unrelated feature work.
- Reactive objects destructured into non-reactive values.
- Watchers used to mirror computed state.
- Deep watch on large data without a bounded source.
- Async watcher results accepted after the owner changes.
- Generic composables hiding router/auth/API/global dependencies.
- DOM refs accessed before mount or used for declarative behavior.
