# Vue Type Contracts

Apply this reference when the accepted Vue stack uses TypeScript and the task owns SFC props/emits/models/slots, composable/store contracts, template refs, injections, directives, plugins, or module augmentation. Do not force a JavaScript project migration.

## SFC Compiler Contract

Keep `vue`, `@vue/compiler-sfc`, language tools, `vue-tsc`, TypeScript, bundler plugin, and test transforms compatible. Macro/type behavior differs across Vue releases.

Use the repository's `<script setup lang="ts">` or conventional `defineComponent` style. Do not mix styles merely to use an external snippet.

Run `vue-tsc` because `tsc` alone does not fully check template/SFC contracts. Preserve project references and generated declaration workflows where used.

## Props, Emits, Models, And Slots

Define props through type-based or runtime declarations according to the repository and runtime validation needs. Keep optional versus nullable exact and use `withDefaults` safely for mutable values.

Use named tuple/call signatures for emits and stable domain payload types. Avoid `any`, broad `Record<string, unknown>`, or unchecked casts at the command boundary.

Use `defineModel` only when the installed Vue/compiler supports it. Type multiple named models/modifiers and preserve invalid intermediate form values.

Type slots with supported SFC macros/tooling when their public props matter; otherwise keep slot usage concrete and verify consumers through `vue-tsc`.

## Reactive Types

Type nullable/replaceable refs explicitly. Let `reactive` infer cohesive object state when possible and do not apply `UnwrapNestedRefs`-style complexity unless a public generic contract requires it.

Use `Ref`, `ComputedRef`, `MaybeRefOrGetter`, or local equivalents in composable contracts only when callers truly pass/react to those forms. Normalize at the boundary and return readonly refs where mutation is not allowed.

Avoid type assertions that claim a template ref, injected dependency, API result, or route param exists before runtime validation.

## Template Refs And Components

Template refs start as null and can return to null. Type DOM refs by element and component refs through public exposed APIs (`defineExpose`/component instance typing) rather than reaching into internals.

For repeated refs, dynamic components, and generic components, follow the installed Vue language-tools support and keep runtime identity checks where types cannot prove the concrete component.

Do not expose entire component instances when one typed command or DOM ref is sufficient.

## Provide, Directives, And Plugins

Use `InjectionKey<T>` and handle missing required providers explicitly. Providing a default object can conceal an installation/configuration defect.

Type custom directives according to element/binding/value/argument/modifier behavior and clean up resources in directive lifecycle hooks.

Declare app global properties and plugin injections with module augmentation in the correct Vue/Nuxt module. Keep server/client availability and optional installation reflected in the type.

## Router And External Data

Type route names/params through the repository router tooling but still validate runtime URL values. Static types do not protect direct external navigation.

Parse API/storage/native payloads at runtime and map them to trusted internal/view types. Generated clients or schemas are preferable to hand-maintained duplicate interfaces when available.

Keep branded IDs, discriminated unions, exact status/error variants, and version fields where they prevent wrong-target or incomplete state handling.

## Generic Components And Composables

Use generics when one real reusable collection/field/selector contract works across types and preserves inference. Avoid generic abstractions that require casts at every template use.

Constrain keys/callbacks and carry stable identity explicitly. A generic list still cannot use index identity for mutable rows.

## Verification

- Run the repository `vue-tsc`/SFC build plus focused consumers after public type changes.
- Include compile fixtures/tests for reusable generic, slot, injection, plugin, or model contracts where supported.
- Exercise runtime parsing for route/API/storage/native values that static types cannot validate.
- Verify nullable refs and optional providers through actual mount/unmount/conditional behavior.
- Ensure generated declarations and package exports remain consumable when library boundaries change.

## Delivery Evidence

Name the SFC/public type boundary, runtime validation boundary, and consumer/build proof. Removing type errors with casts or compiling one component does not establish template, plugin, generated declaration, or external-data safety.

## Unsafe Defaults

- TypeScript migration forced into an accepted JavaScript Vue project.
- `tsc` success used as the only SFC/template proof.
- `any`/casts used at props, emits, params, inject, or API boundaries.
- Template/component refs treated as always initialized.
- Macro APIs used without installed compiler support.
- Generic components introduced despite poor inference and widespread casts.
