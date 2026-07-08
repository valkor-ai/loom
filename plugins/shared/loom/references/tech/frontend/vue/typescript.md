# Vue TypeScript Quality

This file applies Vue TypeScript rules to task-owned components, composables, stores, injected dependencies, template refs, and generic components.

## When To Use

- The task changes typed Vue props, emits, refs, reactive objects, computed values, composables, stores, template refs, generic components, global properties, or Nuxt/Vue module augmentation.
- Use this when type contracts protect component correctness or public feature behavior.
- If the repository is intentionally JavaScript-only, do not force TypeScript migration unless the task owns that migration.

## Implementation Focus

- Use `defineProps<T>()`, `withDefaults()`, and `defineEmits<T>()` for component contracts that need compile-time safety.
- Type nullable refs explicitly, especially API data, selected records, form drafts, template refs, and component refs.
- Avoid destructuring `reactive()` objects unless using `toRefs()` or another established pattern that preserves reactivity.
- Type computed values when inference is unclear or public return shape matters.
- Type composables by their public return contract. Avoid returning loose bags of `any` or implementation-only refs.
- Use generic components only when they simplify a real reusable list, table, picker, or form field contract.
- Use typed `InjectionKey<T>` for provide/inject and fail clearly when required context is missing.
- Type global app properties and Nuxt plugin injections through module augmentation rather than unchecked casts.
- Run `vue-tsc --noEmit` when type contracts are changed.

## Verification Focus

- Run `vue-tsc --noEmit` or the repository's typecheck target.
- Verify component consumers still compile after prop, emit, slot, injection, or store type changes.
- Test runtime validation paths when static types do not cover external input.
- Check template refs are null-safe and only used after mount when DOM/component instances are required.

## Evidence Focus

- In the evidence summary, name the type decision: typed props, typed emits, nullable ref, `toRefs`, composable return type, generic component, injection key, module augmentation, or typecheck proof.
