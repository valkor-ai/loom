# Vue Core Quality

This file applies Vue 3 Composition API discipline to task-owned single-file components, composables, reactive state, and Vue application surfaces.

## When To Use

- The task creates or changes Vue 3 components, composables, reactive state, lifecycle behavior, routing surfaces, forms, lists, or feature screens.
- Use this for Composition API, `<script setup>`, reactivity choices, lifecycle cleanup, composable boundaries, and product UI behavior.
- If the task only edits CSS tokens or visual layout, use UIX references as the authority and keep this file focused on Vue implementation mechanics.

## Implementation Focus

- Prefer `<script setup lang="ts">` and Composition API for new Vue work unless the repository is intentionally Options API and the task must preserve it.
- Use `ref()` for primitives, nullable values, template refs, and replaceable objects. Use `reactive()` for cohesive object state that is not destructured without `toRefs()`.
- Use `computed()` for derived state. Use `watch()` or `watchEffect()` only for side effects, external synchronization, or async refresh behavior.
- Keep lifecycle work explicit. Add cleanup for timers, event listeners, observers, subscriptions, native bridges, and async callbacks that can outlive the component.
- Extract reusable stateful behavior into composables with a small typed return contract. Do not hide product-specific business rules inside generic composable names.
- Keep editable form drafts, selected row snapshots, API data, view filters, and optimistic state separate when they have different lifecycles.
- Do not mutate props directly. Emit typed events or use controlled `v-model` contracts for parent-owned updates.
- Keep product UI free of delivery notes, framework explanations, runtime commands, and verification instructions.

## Verification Focus

- Run `vue-tsc --noEmit`, the repository build, lint, and focused component/composable tests when available.
- Test loading, empty, ready, validation error, business-blocking error, submitting, success, and disabled states touched by the task.
- Verify watcher cleanup, event listener cleanup, stale request prevention, and selected-record action targeting when those risks exist.
- Verify no Options API or Vuex patterns are introduced into a Composition API codebase unless explicitly required by existing code.

## Evidence Focus

- In the evidence summary, name the Vue decision: ref/reactive choice, computed versus watch, lifecycle cleanup, composable boundary, form draft separation, typed emit, or Composition API proof.
