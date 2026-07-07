# Vue Testing Quality

This file applies Vue testing rules to task-owned components, composables, Pinia stores, router behavior, Nuxt pages, async UI, and build-time type contracts.

## When To Use

- The task creates or changes Vue components, composables, stores, forms, route guards, async data, Nuxt pages, or tests.
- Use this alongside Vue Test Utils, Vitest, `vue-tsc`, and the repository's existing testing style.
- If no Vue test infrastructure exists, run available build/type/lint checks and record the gap instead of adding an unrelated test stack for one task.

## Implementation Focus

- Test user-visible behavior: rendered text, labels, roles, disabled state, emitted events, submitted payloads, validation errors, and business-blocking states.
- Mount components with required plugins: Pinia, router, i18n, UI library, query/client utilities, and app providers. Keep test harness setup close to existing project conventions.
- For Pinia stores, use isolated Pinia instances and reset state between tests.
- For composables, test the public return contract through a small component or the repository's composable test helper.
- Mock network and native boundaries at owned adapters. Do not mock the component or store under test.
- For async UI, wait for visible outcomes and promise flushes. Avoid arbitrary sleeps.
- Test cleanup when watchers, timers, event listeners, native listeners, or service workers are part of the change.
- Pair runtime tests with `vue-tsc --noEmit` when typed props, emits, stores, template refs, or module augmentation changed.

## Verification Focus

- Run focused component/store/composable tests plus typecheck/build for changed Vue surfaces.
- Verify success, loading, empty, validation failure, business failure, and unexpected error states when feasible.
- Verify router navigation, route guards, query/param changes, and Nuxt middleware behavior when touched.
- Verify emitted event payloads and `v-model` update contracts for reusable components.

## Evidence Focus

- In the evidence summary, name the proof type: component interaction, emitted event, `v-model` contract, composable contract, Pinia store, router guard, Nuxt page, cleanup proof, or typecheck proof.
