# Vue Component Quality

This file applies Vue component-contract rules to task-owned props, emits, slots, `v-model`, provide/inject, Teleport, dynamic components, and async components.

## When To Use

- The task changes Vue component APIs, props, emitted events, slots, scoped slots, `v-model`, modal/overlay rendering, dynamic components, or reusable UI components.
- Use this when component boundaries and parent-child contracts affect correctness or reuse.
- If the task only changes a private component implementation with no contract impact, keep this file secondary to Vue core rules.

## Implementation Focus

- Define typed props with `defineProps<T>()` and defaults with `withDefaults()` when needed. Keep optional props truly optional in runtime behavior.
- Define typed emits with `defineEmits<T>()`. Do not emit loosely shaped objects when the parent workflow depends on specific fields.
- Use `v-model` for controlled two-way input contracts; name multiple models explicitly and avoid mutating props to simulate two-way binding.
- Use slots for layout extension and scoped slots when the child owns data that the parent must render. Keep slot props small and typed.
- Use provide/inject for stable cross-tree dependencies, not routine prop drilling. Prefer typed `InjectionKey<T>` and readonly state where consumers should not mutate.
- Use Teleport for modals, toasts, command palettes, and overlays that need DOM placement outside the component hierarchy. Preserve focus, labels, and close behavior.
- Use dynamic components and `KeepAlive` only when preserving component state across view switches is part of the product behavior.
- Use `defineAsyncComponent` or route-level lazy loading for heavy optional areas, with loading and error states.
- Keep reusable components domain-neutral only when they are genuinely reusable. Do not generalize a one-off business component prematurely.

## Verification Focus

- Test props, emitted payloads, `v-model` updates, slot rendering, disabled states, and accessible labels touched by the task.
- Verify modal/Teleport focus behavior, close behavior, and action targeting.
- For async components, verify loading and error states as well as the loaded state.
- Run typecheck so prop, emit, slot, and injection contracts stay aligned.

## Evidence Focus

- In the evidence summary, name the component decision: typed props, typed emits, `v-model`, scoped slot, provide/inject, Teleport, dynamic component, async component, or reusable boundary.
